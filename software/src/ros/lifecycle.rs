//! Lifecycle semantics without framework lifecycle (draft 5.3, P4 turn 48).
//!
//! rclrs has no managed-lifecycle node type, so the node implements
//! the same four states (unconfigured / inactive / active / finalized)
//! with the standard transition names, exposed as a `~/transition`
//! service and reflected on `/idong/status`. Allocation happens at
//! configure (channels, buffers, message capacity), USB discovery at
//! activate, teardown at shutdown - the ros2_ouster bring-up order.
//! When rclrs gains native lifecycle support this whole module swaps
//! out without touching the rest of the node.
//!
//! This file is the pure decision core: [`step`] is a total function
//! over (state, transition); the glue owns the side effects.

use core::fmt;

/// The four managed-lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    /// Freshly created; no resources allocated.
    Unconfigured,
    /// Configured; resources allocated, no device I/O.
    Inactive,
    /// Streaming and publishing.
    Active,
    /// Terminal; nothing may run anymore.
    Finalized,
}

impl LifecycleState {
    /// Lowercase name for `/idong/status` and log lines.
    pub fn as_str(self) -> &'static str {
        match self {
            LifecycleState::Unconfigured => "unconfigured",
            LifecycleState::Inactive => "inactive",
            LifecycleState::Active => "active",
            LifecycleState::Finalized => "finalized",
        }
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One standard lifecycle transition (the `~/transition` service
/// payload; the names are the ROS 2 lifecycle standard).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Configure,
    Cleanup,
    Activate,
    Deactivate,
    Shutdown,
    Destroy,
}

impl Transition {
    /// Parse the service string ("configure", "activate", ...).
    pub fn parse(s: &str) -> Option<Transition> {
        match s.trim().to_ascii_lowercase().as_str() {
            "configure" => Some(Transition::Configure),
            "cleanup" => Some(Transition::Cleanup),
            "activate" => Some(Transition::Activate),
            "deactivate" => Some(Transition::Deactivate),
            "shutdown" => Some(Transition::Shutdown),
            "destroy" => Some(Transition::Destroy),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Transition::Configure => "configure",
            Transition::Cleanup => "cleanup",
            Transition::Activate => "activate",
            Transition::Deactivate => "deactivate",
            Transition::Shutdown => "shutdown",
            Transition::Destroy => "destroy",
        }
    }
}

/// An illegal transition request (the error the `~/transition` service
/// returns; the state is carried for the status line).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: LifecycleState,
    pub requested: Transition,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "transition {} is not valid from state {}",
            self.requested.as_str(),
            self.from.as_str()
        )
    }
}

impl std::error::Error for InvalidTransition {}

/// The standard transition table, as a total function:
///
/// * unconfigured + configure -> inactive
/// * inactive + activate -> active
/// * active + deactivate -> inactive
/// * inactive + cleanup -> unconfigured
/// * any live state (unconfigured/inactive/active) + shutdown ->
///   finalized
/// * finalized + destroy -> finalized (idempotent teardown)
///
/// Everything else is an [`InvalidTransition`].
pub fn step(from: LifecycleState, t: Transition) -> Result<LifecycleState, InvalidTransition> {
    use LifecycleState::*;
    let next = match (from, t) {
        (Unconfigured, Transition::Configure) => Inactive,
        (Inactive, Transition::Activate) => Active,
        (Active, Transition::Deactivate) => Inactive,
        (Inactive, Transition::Cleanup) => Unconfigured,
        (Unconfigured | Inactive | Active, Transition::Shutdown) => Finalized,
        (Finalized, Transition::Destroy) => Finalized,
        _ => {
            return Err(InvalidTransition { from, requested: t });
        }
    };
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_standard_table_holds() {
        let cases = [
            (LifecycleState::Unconfigured, Transition::Configure, LifecycleState::Inactive),
            (LifecycleState::Inactive, Transition::Activate, LifecycleState::Active),
            (LifecycleState::Active, Transition::Deactivate, LifecycleState::Inactive),
            (LifecycleState::Inactive, Transition::Cleanup, LifecycleState::Unconfigured),
            (LifecycleState::Unconfigured, Transition::Shutdown, LifecycleState::Finalized),
            (LifecycleState::Inactive, Transition::Shutdown, LifecycleState::Finalized),
            (LifecycleState::Active, Transition::Shutdown, LifecycleState::Finalized),
            (LifecycleState::Finalized, Transition::Destroy, LifecycleState::Finalized),
        ];
        for (from, t, want) in cases {
            assert_eq!(step(from, t).unwrap(), want, "{from} + {t:?}");
        }
    }

    #[test]
    fn every_illegal_pair_is_rejected() {
        use LifecycleState::*;
        let states = [Unconfigured, Inactive, Active, Finalized];
        let transitions = [
            Transition::Configure,
            Transition::Cleanup,
            Transition::Activate,
            Transition::Deactivate,
            Transition::Shutdown,
            Transition::Destroy,
        ];
        let legal = |s, t| step(s, t).is_ok();
        // exactly the eight legal pairs (see the table above)
        for s in states {
            for t in transitions {
                let ok = legal(s, t);
                let expected = matches!(
                    (s, t),
                    (Unconfigured, Transition::Configure)
                        | (Inactive, Transition::Activate)
                        | (Active, Transition::Deactivate)
                        | (Inactive, Transition::Cleanup)
                        | (Unconfigured | Inactive | Active, Transition::Shutdown)
                        | (Finalized, Transition::Destroy)
                );
                assert_eq!(ok, expected, "{s} + {t:?} legality mismatch");
                if !expected {
                    let err = step(s, t).unwrap_err();
                    assert_eq!(err.from, s);
                    assert_eq!(err.requested, t);
                }
            }
        }
    }

    #[test]
    fn transition_parses_and_names_roundtrip() {
        for t in [
            Transition::Configure,
            Transition::Cleanup,
            Transition::Activate,
            Transition::Deactivate,
            Transition::Shutdown,
            Transition::Destroy,
        ] {
            assert_eq!(Transition::parse(t.as_str()), Some(t));
            assert_eq!(Transition::parse(&t.as_str().to_uppercase()), Some(t));
        }
        assert_eq!(Transition::parse("  configure "), Some(Transition::Configure));
        assert_eq!(Transition::parse("reboot"), None);
    }

    #[test]
    fn a_full_bring_up_and_teardown_walks_the_states() {
        // the launch-file walk: configure -> activate -> deactivate ->
        // shutdown, and a destroy on the terminal state.
        let mut s = LifecycleState::Unconfigured;
        s = step(s, Transition::Configure).unwrap();
        assert_eq!(s.as_str(), "inactive");
        s = step(s, Transition::Activate).unwrap();
        assert_eq!(s.as_str(), "active");
        s = step(s, Transition::Deactivate).unwrap();
        assert_eq!(s.as_str(), "inactive");
        s = step(s, Transition::Shutdown).unwrap();
        assert_eq!(s.as_str(), "finalized");
        s = step(s, Transition::Destroy).unwrap();
        assert_eq!(s.as_str(), "finalized");
        // and nothing is legal from finalized except destroy
        assert!(step(s, Transition::Configure).is_err());
    }
}
