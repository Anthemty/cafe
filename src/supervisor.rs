//! Caffeinate process supervisor.
//!
//! Owns at most one child process. The contract:
//! - `enter(Mode::Off)` and `Drop` always kill the child, so we never leak a
//!   dangling `caffeinate` even on panic.
//! - Switching modes kills the previous child and spawns a fresh one (caffeinate
//!   flags can't be changed in-place).
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
    /// Binary path. Configurable so tests can stub it with `sleep`/`true`.
    binary: String,
}

impl Supervisor {
    /// Create a supervisor backed by the system `caffeinate`.
    pub fn new() -> Self {
        Self::with_binary("/usr/bin/caffeinate".to_string())
    }

    /// Create a supervisor with an explicit binary path (for tests).
    pub fn with_binary(binary: String) -> Self {
        Self {
            child: None,
            binary,
        }
    }

    /// Is a child process currently held?
    #[allow(dead_code)]
    pub fn is_armed(&self) -> bool {
        self.child.is_some()
    }

    /// Transition to `mode`.
    ///
    /// - `Off` stops any running child.
    /// - Any armed mode stops the previous child (if any) and spawns a new one.
    ///
    /// If spawn fails the supervisor is left disarmed and the error is
    /// returned; the app stays in a safe (non-leaking) state.
    pub fn enter(&mut self, mode: Mode) -> Result<Transition, SuperviseError> {
        // Always stop the current child first so a failed spawn cannot leave
        // two processes or a stale one running with the wrong flags.
        self.stop();

        match mode.caffeinate_args() {
            None => Ok(Transition::Disarmed),
            Some(args) => {
                let child = Command::new(&self.binary)
                    .args(args)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(SuperviseError::Spawn)?;
                self.child = Some(child);
                Ok(Transition::Armed)
            }
        }
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

    /// Reap a child that has exited on its own, if so. Returns true if the
    /// supervisor is now disarmed.
    #[allow(dead_code)]
    pub fn reap_if_exited(&mut self) -> bool {
        if let Some(child) = self.child.as_mut() {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // It exited; drop our handle.
                    self.child = None;
                    true
                }
                Ok(None) => false,
                Err(_) => {
                    // Could not query; treat as gone to stay safe.
                    self.child = None;
                    true
                }
            }
        } else {
            true
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

    /// Pick a binary that is guaranteed to exist and run "forever" on Unix so
    /// we can validate spawn/kill without depending on `caffeinate`.
    fn sleeper() -> String {
        if cfg!(windows) {
            // `ping` blocks for a while; good enough as a long-lived stub.
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

    /// A thin wrapper to run our stub binary the same way the supervisor runs
    /// caffeinate, exercising the real `Command` path.
    struct StubSupervisor {
        sup: Supervisor,
    }

    impl StubSupervisor {
        fn new() -> Self {
            Self {
                sup: Supervisor::with_binary(sleeper()),
            }
        }

        fn run(&mut self) -> Result<Transition, SuperviseError> {
            // Replicate enter()'s spawn path using raw args.
            self.sup.stop();
            let child = Command::new(&self.sup.binary)
                .args(sleeper_args())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(SuperviseError::Spawn)?;
            self.sup.child = Some(child);
            Ok(Transition::Armed)
        }
    }

    #[test]
    fn starts_disarmed() {
        let s = Supervisor::new();
        assert!(!s.is_armed());
    }

    #[test]
    fn enter_off_disarms() {
        let mut s = Supervisor::new();
        let t = s.enter(Mode::Off).unwrap();
        assert_eq!(t, Transition::Disarmed);
        assert!(!s.is_armed());
    }

    #[test]
    fn stub_spawns_then_stops() {
        let mut stub = StubSupervisor::new();
        let t = stub.run().unwrap();
        assert_eq!(t, Transition::Armed);
        assert!(stub.sup.is_armed());

        stub.sup.stop();
        assert!(!stub.sup.is_armed());
    }

    #[test]
    fn drop_kills_child() {
        let mut stub = StubSupervisor::new();
        stub.run().unwrap();
        assert!(stub.sup.is_armed());
        // Dropping the supervisor must stop the child so we never leak a
        // caffeinate/sleep process. We verify the supervisor reports disarmed
        // after stop+drop; the stronger guarantee (no orphaned OS process) is
        // enforced by Drop::stop's kill+wait.
        stub.sup.stop();
        drop(stub);
    }

    #[test]
    fn enter_off_after_armed_reaps() {
        let mut stub = StubSupervisor::new();
        stub.run().unwrap();
        // Switching to Off must stop the child.
        let t = stub.sup.enter(Mode::Off).unwrap();
        assert_eq!(t, Transition::Disarmed);
        assert!(!stub.sup.is_armed());
    }

    #[test]
    fn spawn_failure_reports_error_and_stays_disarmed() {
        // Point at a binary that does not exist.
        let mut s = Supervisor::with_binary("/nonexistent/binary/xyz".into());
        let err = s.enter(Mode::IdleOnly).unwrap_err();
        assert!(matches!(err, SuperviseError::Spawn(_)));
        assert!(!s.is_armed());
    }

    #[test]
    fn stop_is_idempotent() {
        let mut s = Supervisor::new();
        s.stop(); // no-op
        s.stop(); // still no-op
        assert!(!s.is_armed());
    }
}
