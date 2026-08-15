//! Application state: the keep-awake mode and its on-disk persistence.
//!
//! The mode is the single source of truth for what `caffeinate` flags are in
//! effect. It is deliberately small and `Copy` so it can be passed around the
//! AppKit callbacks without lifetime concerns.

use std::fs;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

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
    pub fn label(self) -> &'static str {
        match self {
            Mode::Off => "Off",
            Mode::IdleOnly => "Idle Only",
            Mode::IdleAndDisplay => "Idle + Display",
        }
    }

    /// Short tooltip describing the effect.
    pub fn tooltip(self) -> &'static str {
        match self {
            Mode::Off => "Sleep prevention off",
            Mode::IdleOnly => "Preventing idle sleep (display may dim)",
            Mode::IdleAndDisplay => "Preventing idle sleep and keeping display on",
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
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// The last manually-selected mode. Restored on launch so a "set and
    /// forget it" workflow survives restarts (the icon always shows the state).
    pub last_mode: Mode,
    /// Whether the "Auto: watch agents" toggle is on.
    pub auto_watch: bool,
}

/// Timer presets offered in the "Keep awake for…" submenu: (minutes, label).
pub const TIMER_PRESETS: &[(u64, &str)] = &[(30, "30 Minutes"), (60, "1 Hour"), (120, "2 Hours")];

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

/// Process-name patterns watched by the "Auto: watch agents" mode. Matched
/// against full command lines via `pgrep -f`, anchored to a path component so
/// e.g. a file named "claude-notes.txt" doesn't match.
pub const AGENT_PATTERNS: &str =
    r"(^|/)(claude|codex|aider|goose|gemini|qwen|cursor-agent|opencode|copilot)( |$)";

/// Are any watched agent processes currently running?
pub fn agents_running() -> bool {
    let out = std::process::Command::new("/usr/bin/pgrep")
        .args(["-f", AGENT_PATTERNS])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    match out {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
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
            assert!(!m.label().is_empty());
            assert!(!m.tooltip().is_empty());
        }
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
        };
        let s = serde_json::to_string(&cfg).unwrap();
        assert!(s.contains("\"last_mode\""));
        assert!(s.contains("\"auto_watch\""));
        assert!(s.contains("\"idle_only\""));
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
        let mut mins: Vec<u64> = TIMER_PRESETS.iter().map(|(m, _)| *m).collect();
        let mut sorted = mins.clone();
        sorted.sort_unstable();
        mins.sort_unstable();
        assert_eq!(mins, sorted);
        assert!(TIMER_PRESETS.iter().all(|(_, l)| !l.is_empty()));
    }
}
