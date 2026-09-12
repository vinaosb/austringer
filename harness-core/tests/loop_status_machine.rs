//! Session-status state machine (todo 5): the complete legal/illegal
//! transition table, terminal absorption, and the single sanctioned
//! terminal escape (Cancelled -> InProgress on resume).

mod common;

use harness_core::agent_loop::{IllegalTransition, SessionStatus};

use SessionStatus::{
    Blocked as B, Cancelled as Cd, Cancelling as Cg, Failed as F, InProgress as I, Success as S,
};

const ALL: [SessionStatus; 6] = [I, B, Cg, Cd, S, F];

/// The exact legal set: run-flow completions/failures/blocking, cancel
/// entry, the cancel terminal, and the resume escape.
fn is_legal(from: SessionStatus, to: SessionStatus) -> bool {
    matches!(
        (from, to),
        (
            SessionStatus::InProgress,
            SessionStatus::Success
                | SessionStatus::Failed
                | SessionStatus::Blocked
                | SessionStatus::Cancelling
        ) | (
            SessionStatus::Blocked,
            SessionStatus::InProgress | SessionStatus::Cancelling
        ) | (SessionStatus::Cancelling, SessionStatus::Cancelled)
            | (SessionStatus::Cancelled, SessionStatus::InProgress)
    )
}

#[test]
fn transition_table_matches_the_legal_set_exactly() {
    for from in ALL {
        for to in ALL {
            let got = from.transition(to);
            if is_legal(from, to) {
                assert_eq!(got, Ok(to), "expected {from:?} -> {to:?} to be legal");
            } else {
                assert_eq!(
                    got,
                    Err(IllegalTransition { from, to }),
                    "expected {from:?} -> {to:?} to be rejected"
                );
            }
        }
    }
}

#[test]
fn success_and_failed_are_absorbing_terminal_states() {
    assert!(S.is_terminal());
    assert!(F.is_terminal());
    for to in ALL {
        assert!(S.transition(to).is_err());
        assert!(F.transition(to).is_err());
    }
}

#[test]
fn cancelled_is_terminal_except_for_the_resume_escape() {
    assert!(Cd.is_terminal());
    assert_eq!(Cd.transition(I), Ok(I));
    for to in [B, Cg, Cd, S, F] {
        assert!(Cd.transition(to).is_err());
    }
}

#[test]
fn cancelling_may_only_land_on_cancelled() {
    for to in [I, B, Cg, S, F] {
        assert!(Cg.transition(to).is_err());
    }
    assert_eq!(Cg.transition(Cd), Ok(Cd));
}
