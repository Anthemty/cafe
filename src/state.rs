//! Application state: the keep-awake mode and its on-disk persistence.
//!
//! The mode is the single source of truth for what `caffeinate` flags are in
//! effect. It is deliberately small and `Copy` so it can be passed around the
//! AppKit callbacks without lifetime concerns.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// UI language. Defaults to English; a manual selection in the menu overrides
/// and persists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    #[default]
    En,
    Zh,
}

impl Lang {
    /// Cycle for the menu toggle: En → Zh → En.
    pub fn next(self) -> Lang {
        match self {
            Lang::En => Lang::Zh,
            Lang::Zh => Lang::En,
        }
    }

    /// Label shown on the Language menu item (always shows the OTHER language's
    /// name, i.e. what you'd switch to).
    pub fn toggle_label(self) -> &'static str {
        match self {
            Lang::En => "语言:中文",
            Lang::Zh => "Language: English",
        }
    }
}

/// Which level of sleep prevention is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// No sleep prevention. `caffeinate` is not running.
    Off,
    /// Prevent idle system sleep only. Display may dim. `caffeinate -i`.
    IdleOnly,
    /// Prevent idle system sleep and keep the display awake. `caffeinate -di`.
    IdleAndDisplay,
}

impl Mode {
    /// All modes in menu order.
    pub const ALL: [Mode; 3] = [Mode::Off, Mode::IdleOnly, Mode::IdleAndDisplay];

    /// Human-readable label for the menu.
    pub fn label(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Mode::Off, Lang::En) => "Off",
            (Mode::Off, Lang::Zh) => "关闭",
            (Mode::IdleOnly, Lang::En) => "Idle Only",
            (Mode::IdleOnly, Lang::Zh) => "仅防休眠",
            (Mode::IdleAndDisplay, Lang::En) => "Idle + Display",
            (Mode::IdleAndDisplay, Lang::Zh) => "防休眠 + 常亮",
        }
    }

    /// Short tooltip describing the effect.
    pub fn tooltip(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Mode::Off, Lang::En) => "Sleep prevention off",
            (Mode::Off, Lang::Zh) => "未开启防休眠",
            (Mode::IdleOnly, Lang::En) => "Preventing idle sleep (display may dim)",
            (Mode::IdleOnly, Lang::Zh) => "防止系统休眠(屏幕可能变暗)",
            (Mode::IdleAndDisplay, Lang::En) => "Preventing idle sleep and keeping display on",
            (Mode::IdleAndDisplay, Lang::Zh) => "防止系统休眠且屏幕常亮",
        }
    }

    /// Command-line arguments to pass to `caffeinate`, or `None` when off.
    pub fn caffeinate_args(self) -> Option<&'static [&'static str]> {
        match self {
            Mode::Off => None,
            Mode::IdleOnly => Some(&["-i"]),
            Mode::IdleAndDisplay => Some(&["-di"]),
        }
    }

    /// Next mode in the global-hotkey cycle: Off → IdleOnly → IdleAndDisplay.
    pub fn next_in_cycle(self) -> Mode {
        match self {
            Mode::Off => Mode::IdleOnly,
            Mode::IdleOnly => Mode::IdleAndDisplay,
            Mode::IdleAndDisplay => Mode::Off,
        }
    }
}

impl Default for Mode {
    /// Safe default: do nothing until the user opts in.
    fn default() -> Self {
        Mode::Off
    }
}

/// On-disk config, persisted to `~/Library/Application Support/cafe/config.json`.
///
/// Fields added after 0.1.0 use `#[serde(default)]` so older config files
/// (missing the new keys) still deserialize instead of being wiped by the
/// corrupt-file fallback.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The last manually-selected mode. Restored on launch so a "set and
    /// forget it" workflow survives restarts (the icon always shows the state).
    #[serde(default)]
    pub last_mode: Mode,
    /// Whether the "Auto: watch agents" toggle is on.
    #[serde(default)]
    pub auto_watch: bool,
    /// UI language (menu/tooltip strings).
    #[serde(default)]
    pub lang: Lang,
}

/// Timer preset durations in minutes. Labels are per-language (see
/// `timer_label`).
pub const TIMER_PRESETS: &[u64] = &[30, 60, 120];

/// Menu label for a timer preset in the given language.
pub fn timer_label(minutes: u64, lang: Lang) -> String {
    match (minutes, lang) {
        (30, Lang::En) => "30 Minutes".into(),
        (30, Lang::Zh) => "30 分钟".into(),
        (60, Lang::En) => "1 Hour".into(),
        (60, Lang::Zh) => "1 小时".into(),
        (120, Lang::En) => "2 Hours".into(),
        (120, Lang::Zh) => "2 小时".into(),
        (other, Lang::En) => format!("{other} Minutes"),
        (other, Lang::Zh) => format!("{other} 分钟"),
    }
}

/// Countdown suffix for the status tooltip, e.g. "42 min left" / "剩余 42 分钟".
pub fn countdown_text(minutes_left: u64, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{minutes_left} min left"),
        Lang::Zh => format!("剩余 {minutes_left} 分钟"),
    }
}

/// Compact clock for the menu bar / menu header: `m:ss` under an hour,
/// `h:mm:ss` above. Always counts real seconds so it visibly ticks.
pub fn format_clock(total_secs: u64) -> String {
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Upper bound for custom timed sessions (24 h).
pub const TIMER_MAX_MINUTES: u64 = 1440;

/// Parse the custom-timer input: trimmed positive integer, capped at
/// `TIMER_MAX_MINUTES`. Returns `None` on anything else (caller ignores the
/// request silently).
pub fn parse_custom_minutes(input: &str) -> Option<u64> {
    let t = input.trim();
    let n: u64 = t.parse().ok()?;
    (1..=TIMER_MAX_MINUTES).contains(&n).then_some(n)
}

/// Which mode a timed session should arm/keep:
/// - currently armed → keep the current mode;
/// - currently off → the last manually armed mode;
/// - never armed → `IdleOnly` (gentlest sensible default).
pub fn timer_target(current: Mode, last_armed: Mode) -> Mode {
    match current {
        Mode::Off => match last_armed {
            Mode::Off => Mode::IdleOnly,
            other => other,
        },
        other => other,
    }
}

/// Where the config file lives: `~/Library/Application Support/cafe/config.json`.
fn config_path() -> Option<PathBuf> {
    dirs_support_dir().map(|d| d.join("cafe").join("config.json"))
}

#[cfg(target_os = "macos")]
fn dirs_support_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
}

#[cfg(not(target_os = "macos"))]
fn dirs_support_dir() -> Option<PathBuf> {
    None
}

/// Load config. Missing/corrupt file is non-fatal — falls back to default.
pub fn load_config() -> Config {
    let Some(path) = config_path() else {
        return Config::default();
    };
    match fs::read_to_string(&path) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

/// Persist config. Failure is non-fatal (read-only home, sandbox, etc.).
pub fn save_config(cfg: &Config) -> io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = serde_json::to_string_pretty(cfg).map_err(io::Error::other)?;
    fs::write(path, s)
}

/// Append a line to `~/Library/Application Support/cafe/panic.log`. Used by the
/// panic hook: a menu bar app has no visible stderr, so panics must land
/// somewhere the user (or a bug report) can find them. Failures are ignored.
pub fn append_panic_log(message: &str) {
    let Some(path) = config_path().map(|p| p.with_file_name("panic.log")) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let stamped = format!("[{}] {}\n", chrono_now(), message);
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(stamped.as_bytes());
    }
}

/// Local-time-ish timestamp without external crates: seconds since epoch as a
/// stable identifier (good enough to order panic entries).
fn chrono_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The LaunchAgent plist path used for "Launch at Login"
/// (`~/Library/LaunchAgents/dev.cafe.app.plist`).
pub fn launch_agent_path() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join("Library")
            .join("LaunchAgents")
            .join("dev.cafe.app.plist")
    })
}

/// Is the launch-at-login item installed (plist present)?
pub fn login_item_enabled() -> bool {
    launch_agent_path().is_some_and(|p| p.exists())
}

/// Install or remove the launch-at-login item. Returns an error message on
/// write failure so the UI can surface it.
pub fn set_login_item(enabled: bool) -> Result<(), String> {
    let Some(path) = launch_agent_path() else {
        return Err("cannot determine home directory".into());
    };
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|e| format!("remove plist: {e}"))?;
        }
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let exe = exe.to_string_lossy().replace('\'', "'\\''");
    let plist = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
<plist version=\"1.0\">\n\
<dict>\n\
    <key>Label</key>\n\
    <string>dev.cafe.app</string>\n\
    <key>ProgramArguments</key>\n\
    <array>\n\
        <string>{exe}</string>\n\
    </array>\n\
    <key>RunAtLoad</key>\n\
    <true/>\n\
    <key>ProcessType</key>\n\
    <string>Background</string>\n\
</dict>\n\
</plist>\n"
    );
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create LaunchAgents dir: {e}"))?;
    }
    fs::write(&path, plist).map_err(|e| format!("write plist: {e}"))
}

/// Agents shown in the menu (lit = process running) and watched by auto mode:
/// `(process basename, menu label)`. Matching is case-insensitive so both
/// `claude` (CLI) and `Claude` (desktop app executable) light the same entry.
pub const AGENTS: &[(&str, &str)] = &[
    ("claude", "Claude"),
    ("codex", "Codex"),
    ("workbuddy", "WorkBuddy"),
    ("zcode", "ZCode"),
    ("opencode", "OpenCode"),
];

/// True when `line` (one `ps -axo command=` row) is running agent `name`:
/// some whitespace-separated token's basename equals `name`, case-insensitively.
/// Anchoring on basename avoids false hits like `claude-sonnet-5` args or
/// `claude-notes.md` file arguments, while still matching
/// `/Applications/Claude.app/Contents/MacOS/Claude`.
pub fn cmdline_matches(line: &str, name: &str) -> bool {
    line.split_whitespace().any(|tok| {
        tok.rsplit('/')
            .next()
            .unwrap_or(tok)
            .eq_ignore_ascii_case(name)
    })
}

/// One-shot scan of the process table. Returns online state per entry of
/// `AGENTS` (same order).
pub fn detect_agents() -> Vec<bool> {
    // NOTE: do NOT set `.stdout(Stdio::null())` here — `output()` captures
    // whatever stdout points at, and null yields an empty buffer.
    let text = match std::process::Command::new("/bin/ps")
        .args(["-axo", "command="])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return vec![false; AGENTS.len()],
    };
    AGENTS
        .iter()
        .map(|(name, _)| text.lines().any(|l| cmdline_matches(l, name)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_labels_and_args_are_consistent() {
        assert_eq!(Mode::Off.caffeinate_args(), None);
        assert_eq!(Mode::IdleOnly.caffeinate_args(), Some(&["-i"][..]));
        assert_eq!(Mode::IdleAndDisplay.caffeinate_args(), Some(&["-di"][..]));
        for m in Mode::ALL {
            for lang in [Lang::En, Lang::Zh] {
                assert!(!m.label(lang).is_empty());
                assert!(!m.tooltip(lang).is_empty());
            }
        }
    }

    #[test]
    fn lang_cycles_and_labels() {
        assert_eq!(Lang::En.next(), Lang::Zh);
        assert_eq!(Lang::Zh.next(), Lang::En);
        assert_eq!(Lang::En.toggle_label(), "语言:中文");
        assert_eq!(Lang::Zh.toggle_label(), "Language: English");
    }

    #[test]
    fn cycle_order_is_off_idle_display() {
        assert_eq!(Mode::Off.next_in_cycle(), Mode::IdleOnly);
        assert_eq!(Mode::IdleOnly.next_in_cycle(), Mode::IdleAndDisplay);
        assert_eq!(Mode::IdleAndDisplay.next_in_cycle(), Mode::Off);
    }

    #[test]
    fn config_serde_roundtrip() {
        let cfg = Config {
            last_mode: Mode::IdleAndDisplay,
            auto_watch: true,
            lang: Lang::Zh,
        };
        let s = serde_json::to_string(&cfg).unwrap();
        let back: Config = serde_json::from_str(&s).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn config_snake_case_field_names() {
        let cfg = Config {
            last_mode: Mode::IdleOnly,
            auto_watch: false,
            lang: Lang::En,
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("\"last_mode\""));
        assert!(s.contains("\"auto_watch\""));
        assert!(s.contains("\"lang\""));
        assert!(s.contains("\"idle_only\""));
    }

    #[test]
    fn missing_lang_field_defaults_to_english() {
        // Old configs (v0.2.0) have no "lang" — serde default keeps them valid.
        let cfg: Config =
            serde_json::from_str("{\"last_mode\":\"off\",\"auto_watch\":false}").unwrap();
        assert_eq!(cfg.lang, Lang::En);
    }

    #[test]
    fn corrupt_config_falls_back_to_default() {
        let bad: Result<Config, _> = serde_json::from_str("{ not json");
        assert!(bad.is_err());
        // The same fallback logic load_config applies:
        assert_eq!(
            serde_json::from_str::<Config>("{ not json").unwrap_or_default(),
            Config::default()
        );
    }

    #[test]
    fn timer_presets_are_sorted_and_labeled() {
        let mut mins = TIMER_PRESETS.to_vec();
        let mut sorted = mins.clone();
        sorted.sort_unstable();
        mins.sort_unstable();
        assert_eq!(mins, sorted);
        for &m in TIMER_PRESETS {
            for lang in [Lang::En, Lang::Zh] {
                assert!(!timer_label(m, lang).is_empty());
            }
        }
    }

    #[test]
    fn countdown_text_localizes() {
        assert_eq!(countdown_text(42, Lang::En), "42 min left");
        assert_eq!(countdown_text(42, Lang::Zh), "剩余 42 分钟");
    }

    #[test]
    fn clock_formats_seconds_and_hours() {
        assert_eq!(format_clock(0), "0:00");
        assert_eq!(format_clock(59), "0:59");
        assert_eq!(format_clock(60), "1:00");
        assert_eq!(format_clock(12 * 60 + 34), "12:34");
        assert_eq!(format_clock(3600), "1:00:00");
        assert_eq!(format_clock(2 * 3600 + 5 * 60 + 9), "2:05:09");
    }

    #[test]
    fn custom_minutes_parse_bounds() {
        assert_eq!(parse_custom_minutes(" 45 "), Some(45));
        assert_eq!(parse_custom_minutes("1"), Some(1));
        assert_eq!(parse_custom_minutes("1440"), Some(1440)); // 24h cap
        assert_eq!(parse_custom_minutes("1441"), None);
        assert_eq!(parse_custom_minutes("0"), None);
        assert_eq!(parse_custom_minutes("-5"), None);
        assert_eq!(parse_custom_minutes("abc"), None);
        assert_eq!(parse_custom_minutes(""), None);
    }

    #[test]
    fn timer_target_prefers_current_then_last_armed() {
        assert_eq!(timer_target(Mode::IdleOnly, Mode::Off), Mode::IdleOnly);
        assert_eq!(
            timer_target(Mode::IdleAndDisplay, Mode::IdleOnly),
            Mode::IdleAndDisplay
        );
        assert_eq!(
            timer_target(Mode::Off, Mode::IdleAndDisplay),
            Mode::IdleAndDisplay
        );
        assert_eq!(timer_target(Mode::Off, Mode::Off), Mode::IdleOnly);
    }

    #[test]
    fn cmdline_matches_real_process_shapes() {
        // Real shapes seen on this machine:
        assert!(cmdline_matches(
            "/Applications/Claude.app/Contents/MacOS/Claude",
            "claude"
        ));
        assert!(cmdline_matches(
            "/Applications/Claude.app/Contents/Helpers/disclaimer -- /Users/x/claude --effort high",
            "claude"
        ));
        assert!(cmdline_matches(
            "/Applications/ChatGPT.app/Contents/Resources/codex -c app-server",
            "codex"
        ));
        assert!(cmdline_matches("/opt/tools/claude 45", "claude"));
        assert!(cmdline_matches("claude --verbose", "claude"));
        // Case-insensitivity:
        assert!(cmdline_matches("ZCode", "zcode"));
        // Near-misses that must NOT match:
        assert!(!cmdline_matches("zcode-cli --serve", "zcode"));
        assert!(!cmdline_matches("zcode-host-local-1", "zcode"));
        assert!(!cmdline_matches("codex-code-mode-host", "codex"));
        assert!(!cmdline_matches("vim claude-notes.md", "claude"));
        assert!(!cmdline_matches("--model claude-sonnet-5", "claude"));
        assert!(!cmdline_matches("claude.app", "claude"));
        assert!(!cmdline_matches("", "claude"));
    }

    #[test]
    fn agents_table_is_wellformed_and_detect_sized() {
        assert_eq!(AGENTS.len(), 5);
        assert!(AGENTS.iter().all(|(n, l)| !n.is_empty() && !l.is_empty()));
        let online = detect_agents();
        assert_eq!(online.len(), AGENTS.len());
    }
}
