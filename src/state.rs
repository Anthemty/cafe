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
}

impl Default for Mode {
    /// Safe default: do nothing until the user opts in.
    fn default() -> Self {
        Mode::Off
    }
}

/// On-disk config. Only `last_mode` is remembered (for UX continuity); the app
/// still starts in `Off` and the user must re-arm it.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub last_mode: Mode,
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
