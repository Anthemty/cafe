//! Caffeinate process supervisor.
//!
//! Owns at most one child process. The contract:
//! - `enter(Mode::Off, _)` and `Drop` always kill the child, so we never leak a
//!   dangling `caffeinate` even on panic.
//! - Switching modes kills the previous child and spawns a fresh one (caffeinate
//!   flags can't be changed in-place).
//! - The real supervisor passes `-w <our pid>` so caffeinate watches its parent:
//!   even if cafe is SIGKILLed (no Rust `Drop` runs), caffeinate exits by
//!   itself. Test stubs disable this (their stub binary doesn't know `-w`).
//! - A missing binary is reported as an error rather than silently ignored.

use std::io;
use std::process::{Child, Command, Stdio};

use crate::state::Mode;

/// Errors that can occur when arming the supervisor.
#[derive(Debug)]
pub enum SuperviseError {
    /// `caffeinate` (or the configured binary) could not be found/spawned.
    Spawn(io::Error),
}

impl std::fmt::Display for SuperviseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SuperviseError::Spawn(e) => write!(f, "failed to spawn caffeinate: {e}"),
        }
    }
}

impl std::error::Error for SuperviseError {}

/// Result of a transition: did we end up armed (child running) or disarmed?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// A caffeinate child is now running.
    Armed,
    /// No child is running.
    Disarmed,
}

/// Manages a single `caffeinate` child process.
pub struct Supervisor {
    child: Option<Child>,
    /// Binary path. Configurable so tests can stub it with `sleep`.
    binary: String,
    /// Pass `-w <our pid>` so the child self-terminates if cafe dies. Only the
    /// real supervisor sets this; test stubs use binaries without `-w`.
    watch_parent: bool,
}

impl Supervisor {
    /// Create a supervisor backed by the system `caffeinate`, with parent-watch
    /// enabled (no orphaned caffeinate even on SIGKILL of cafe).
    pub fn new() -> Self {
        Self {
            child: None,
            binary: "/usr/bin/caffeinate".to_string(),
            watch_parent: true,
        }
    }

    /// Create a supervisor with an explicit binary path (for tests). Parent
    /// watching is disabled because stub binaries don't implement `-w`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn with_binary(binary: String) -> Self {
        Self {
            child: None,
            binary,
            watch_parent: false,
        }
    }

    /// Is a child process currently held?
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn is_armed(&self) -> bool {
        self.child.is_some()
    }

    /// Transition to `mode`, optionally limited to `timeout_secs` (passes
    /// `caffeinate -t`; the child self-exits at the deadline).
    ///
    /// - `Off` stops any running child.
    /// - Any armed mode stops the previous child (if any) and spawns a new one.
    ///
    /// If spawn fails the supervisor is left disarmed and the error is
    /// returned; the app stays in a safe (non-leaking) state.
    pub fn enter(
        &mut self,
        mode: Mode,
        timeout_secs: Option<u64>,
    ) -> Result<Transition, SuperviseError> {
        // Always stop the current child first so a failed spawn cannot leave
        // two processes or a stale one running with the wrong flags.
        self.stop();

        let Some(base_args) = mode.caffeinate_args() else {
            return Ok(Transition::Disarmed);
        };

        let mut args: Vec<String> = base_args.iter().map(|s| s.to_string()).collect();
        if let Some(secs) = timeout_secs {
            args.push("-t".into());
            args.push(secs.to_string());
        }
        if self.watch_parent {
            args.push("-w".into());
            args.push(std::process::id().to_string());
        }

        let child = Command::new(&self.binary)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(SuperviseError::Spawn)?;
        self.child = Some(child);
        Ok(Transition::Armed)
    }

    /// Kill and reap the current child, if any. Idempotent.
    pub fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        // Best effort: try to terminate gracefully, then force kill.
        let _ = child.kill();
        let _ = child.wait();
    }

    /// Reap a child that has exited on its own (e.g. a `-t` timeout elapsed, or
    /// someone killed caffeinate from outside). Returns `true` when the
    /// supervisor is now disarmed.
    pub fn reap_if_exited(&mut self) -> bool {
        match self.child.as_mut() {
            None => true,
            Some(child) => match child.try_wait() {
                // It exited; drop our handle.
                Ok(Some(_)) => {
                    self.child = None;
                    true
                }
                Ok(None) => false,
                // Could not query; treat as gone to stay safe.
                Err(_) => {
                    self.child = None;
                    true
                }
            },
        }
    }
}

impl Default for Supervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // Guarantee no leaked caffeinate process on any exit path.
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sleeper() -> String {
        if cfg!(windows) {
            "ping".to_string()
        } else {
            "sleep".to_string()
        }
    }

    fn sleeper_args() -> Vec<String> {
        if cfg!(windows) {
            vec!["-n".into(), "9999".into(), "127.0.0.1".into()]
        } else {
            vec!["9999".into()]
        }
    }

    fn spawn_stub(sup: &mut Supervisor) -> Result<Transition, SuperviseError> {
        let child = Command::new(sup.binary.clone())
            .args(sleeper_args())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(SuperviseError::Spawn)?;
        sup.child = Some(child);
        Ok(Transition::Armed)
    }

    #[test]
    fn starts_disarmed() {
        assert!(!Supervisor::new().is_armed());
    }

    #[test]
    fn enter_off_disarms() {
        let mut s = Supervisor::new();
        assert_eq!(s.enter(Mode::Off, None).unwrap(), Transition::Disarmed);
        assert!(!s.is_armed());
    }

    #[test]
    fn stub_spawns_then_stops() {
        let mut s = Supervisor::with_binary(sleeper());
        assert_eq!(spawn_stub(&mut s).unwrap(), Transition::Armed);
        assert!(s.is_armed());
        s.stop();
        assert!(!s.is_armed());
    }

    #[test]
    fn drop_kills_child() {
        let mut s = Supervisor::with_binary(sleeper());
        spawn_stub(&mut s).unwrap();
        drop(s);
    }

    #[test]
    fn enter_off_after_armed_reaps() {
        let mut s = Supervisor::with_binary(sleeper());
        spawn_stub(&mut s).unwrap();
        assert_eq!(s.enter(Mode::Off, None).unwrap(), Transition::Disarmed);
        assert!(!s.is_armed());
    }

    #[test]
    fn reap_reports_disarm_when_child_exits() {
        // A stub that exits immediately exercises the "child already gone" path.
        let mut s = Supervisor::with_binary("true".into());
        let _ = s.enter(Mode::IdleOnly, None);
        if let Some(mut child) = s.child.take() {
            let _ = child.wait();
        }
        assert!(!s.is_armed());

        // A live child must NOT be reported as exited.
        let mut s2 = Supervisor::with_binary(sleeper());
        let _ = s2.enter(Mode::IdleOnly, None);
        assert!(!s2.reap_if_exited());
        s2.stop();
    }

    #[test]
    fn spawn_failure_reports_error_and_stays_disarmed() {
        let mut s = Supervisor::with_binary("/nonexistent/binary/xyz".into());
        let err = s.enter(Mode::IdleOnly, None).unwrap_err();
        assert!(matches!(err, SuperviseError::Spawn(_)));
        assert!(!s.is_armed());
    }

    #[test]
    fn stop_is_idempotent() {
        let mut s = Supervisor::new();
        s.stop();
        s.stop();
        assert!(!s.is_armed());
    }
}
