//! Recording session state machine. Every state change goes through [`next`], so a session can
//! never be, say, paused before it started or stopped twice. The capture backends (phase 2)
//! drive it; the same states are stored in `capture_sessions.state`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecState {
    Idle,
    Preparing,
    Countdown,
    Recording,
    Paused,
    Finalizing,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Prepare,
    /// Preparation finished; `countdown` = a countdown was requested.
    Ready { countdown: bool },
    CountdownDone,
    Pause,
    Resume,
    Stop,
    Finalized,
    Cancel,
    Fail,
}

impl RecState {
    pub fn as_str(self) -> &'static str {
        match self {
            RecState::Idle => "idle",
            RecState::Preparing => "preparing",
            RecState::Countdown => "countdown",
            RecState::Recording => "recording",
            RecState::Paused => "paused",
            RecState::Finalizing => "finalizing",
            RecState::Completed => "completed",
            RecState::Failed => "failed",
            RecState::Cancelled => "cancelled",
        }
    }

    /// Capture is running or about to (the app must not quit silently).
    pub fn is_active(self) -> bool {
        matches!(self, RecState::Preparing | RecState::Countdown | RecState::Recording | RecState::Paused | RecState::Finalizing)
    }

    pub fn is_final(self) -> bool {
        matches!(self, RecState::Completed | RecState::Failed | RecState::Cancelled)
    }
}

/// The state after `ev`, or `None` when the event is not allowed in `s`.
pub fn next(s: RecState, ev: Event) -> Option<RecState> {
    use Event as E;
    use RecState as S;
    Some(match (s, ev) {
        (S::Idle | S::Completed | S::Failed | S::Cancelled, E::Prepare) => S::Preparing,
        (S::Preparing, E::Ready { countdown: true }) => S::Countdown,
        (S::Preparing, E::Ready { countdown: false }) | (S::Countdown, E::CountdownDone) => S::Recording,
        (S::Recording, E::Pause) => S::Paused,
        (S::Paused, E::Resume) => S::Recording,
        (S::Recording | S::Paused, E::Stop) => S::Finalizing,
        (S::Finalizing, E::Finalized) => S::Completed,
        // Cancel before anything was recorded discards nothing worth keeping.
        (S::Preparing | S::Countdown, E::Cancel) => S::Cancelled,
        (s, E::Fail) if s.is_active() => S::Failed,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use Event as E;
    use RecState as S;

    fn run(events: &[Event]) -> Option<RecState> {
        events.iter().try_fold(S::Idle, |s, e| next(s, *e))
    }

    #[test]
    fn normal_paths() {
        assert_eq!(run(&[E::Prepare, E::Ready { countdown: false }, E::Stop, E::Finalized]), Some(S::Completed));
        assert_eq!(run(&[E::Prepare, E::Ready { countdown: true }, E::CountdownDone, E::Pause, E::Resume, E::Pause, E::Stop, E::Finalized]), Some(S::Completed));
        assert_eq!(run(&[E::Prepare, E::Ready { countdown: true }, E::Cancel]), Some(S::Cancelled));
        // A new recording can start after the previous one ended.
        assert_eq!(run(&[E::Prepare, E::Ready { countdown: false }, E::Stop, E::Finalized, E::Prepare]), Some(S::Preparing));
    }

    #[test]
    fn impossible_transitions() {
        assert_eq!(next(S::Idle, E::Pause), None);
        assert_eq!(next(S::Idle, E::Stop), None);
        assert_eq!(next(S::Paused, E::Pause), None);
        assert_eq!(next(S::Finalizing, E::Stop), None);
        assert_eq!(next(S::Completed, E::Resume), None);
        assert_eq!(next(S::Recording, E::Prepare), None);
        // Cancelling a running recording is a Stop (the file is kept), not a Cancel.
        assert_eq!(next(S::Recording, E::Cancel), None);
        assert_eq!(next(S::Completed, E::Fail), None);
    }

    #[test]
    fn failures_from_every_active_state() {
        for s in [S::Preparing, S::Countdown, S::Recording, S::Paused, S::Finalizing] {
            assert!(s.is_active());
            assert_eq!(next(s, E::Fail), Some(S::Failed));
        }
        assert!(S::Completed.is_final() && !S::Idle.is_active());
    }
}
