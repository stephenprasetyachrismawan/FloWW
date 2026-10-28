//! The main flow as a state machine.
//!
//! Steps follow the design doc (1–19). Human feedback after testing loops
//! back to Develop. Building cannot start before the PRD is approved.

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    NewRequest = 1,
    CheckBrain,
    ReadDatabase,
    ReadCodebase,
    UpdateBrain,
    GrillPrd,
    ApprovePrd,
    CreateBranch,
    Develop,
    FullSuite,
    LogChanges,
    StartOdoo,
    UpgradeModules,
    HumanTest,
    Feedback,
    CommitMessage,
    Commit,
    Push,
    PullRequest,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Request,
    Planning,
    Building,
    Testing,
    Ship,
    Done,
}

/// Who acts at a step. Drives the "Your turn" list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Actor {
    Ai,
    Human,
    App,
}

impl Step {
    pub const ALL: [Step; 20] = [
        Step::NewRequest,
        Step::CheckBrain,
        Step::ReadDatabase,
        Step::ReadCodebase,
        Step::UpdateBrain,
        Step::GrillPrd,
        Step::ApprovePrd,
        Step::CreateBranch,
        Step::Develop,
        Step::FullSuite,
        Step::LogChanges,
        Step::StartOdoo,
        Step::UpgradeModules,
        Step::HumanTest,
        Step::Feedback,
        Step::CommitMessage,
        Step::Commit,
        Step::Push,
        Step::PullRequest,
        Step::Done,
    ];

    pub fn number(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Step::NewRequest => "new_request",
            Step::CheckBrain => "check_brain",
            Step::ReadDatabase => "read_database",
            Step::ReadCodebase => "read_codebase",
            Step::UpdateBrain => "update_brain",
            Step::GrillPrd => "grill_prd",
            Step::ApprovePrd => "approve_prd",
            Step::CreateBranch => "create_branch",
            Step::Develop => "develop",
            Step::FullSuite => "full_suite",
            Step::LogChanges => "log_changes",
            Step::StartOdoo => "start_odoo",
            Step::UpgradeModules => "upgrade_modules",
            Step::HumanTest => "human_test",
            Step::Feedback => "feedback",
            Step::CommitMessage => "commit_message",
            Step::Commit => "commit",
            Step::Push => "push",
            Step::PullRequest => "pull_request",
            Step::Done => "done",
        }
    }

    pub fn phase(self) -> Phase {
        match self.number() {
            1..=5 => Phase::Request,
            6..=7 => Phase::Planning,
            8..=11 => Phase::Building,
            12..=15 => Phase::Testing,
            16..=19 => Phase::Ship,
            _ => Phase::Done,
        }
    }

    /// Grill and commit message are drafted by the AI but wait on a person.
    pub fn actor(self) -> Actor {
        match self {
            Step::NewRequest
            | Step::GrillPrd
            | Step::ApprovePrd
            | Step::HumanTest
            | Step::Feedback
            | Step::CommitMessage
            | Step::Commit
            | Step::Push
            | Step::PullRequest => Actor::Human,
            Step::CheckBrain
            | Step::ReadDatabase
            | Step::ReadCodebase
            | Step::Develop
            | Step::LogChanges => Actor::Ai,
            Step::UpdateBrain
            | Step::CreateBranch
            | Step::FullSuite
            | Step::StartOdoo
            | Step::UpgradeModules
            | Step::Done => Actor::App,
        }
    }

    /// Steps that need the shared local database lock.
    pub fn needs_db_lock(self) -> bool {
        matches!(
            self,
            Step::FullSuite | Step::StartOdoo | Step::UpgradeModules | Step::HumanTest
        )
    }

    fn from_number(n: u8) -> Option<Step> {
        Step::ALL.get(usize::from(n.checked_sub(1)?)).copied()
    }
}

/// Options that change which step comes next.
#[derive(Debug, Clone, Copy, Default)]
pub struct FlowOptions {
    pub full_suite: bool,
    pub open_pull_request: bool,
}

/// The step after `current` in the normal forward flow.
pub fn next_step(current: Step, opts: FlowOptions) -> Option<Step> {
    let next = match current {
        Step::Develop if !opts.full_suite => Step::LogChanges,
        Step::Push if !opts.open_pull_request => Step::Done,
        Step::Done => return None,
        s => Step::from_number(s.number() + 1)?,
    };
    Some(next)
}

/// Check a move requested by the user or the engine.
///
/// Allowed: the normal next step, feedback looping back to Develop, and
/// returning from Approve to Grill to ask more questions.
pub fn check_transition(from: Step, to: Step, opts: FlowOptions) -> Result<()> {
    if next_step(from, opts) == Some(to) {
        return Ok(());
    }
    let ok = matches!(
        (from, to),
        (Step::Feedback, Step::Develop)
            | (Step::HumanTest, Step::Feedback)
            | (Step::ApprovePrd, Step::GrillPrd)
            // The human may skip the PR on the ship page.
            | (Step::Push, Step::Done)
    );
    if ok {
        return Ok(());
    }
    let reason =
        if to.number() > Step::ApprovePrd.number() && from.number() < Step::ApprovePrd.number() {
            "the PRD must be approved first"
        } else if to.number() > from.number() {
            "steps cannot be skipped"
        } else {
            "only feedback can send work back to Develop"
        };
    Err(Error::InvalidTransition {
        from: from.name(),
        to: to.name(),
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_numbered_like_the_design_doc() {
        assert_eq!(Step::NewRequest.number(), 1);
        assert_eq!(Step::Develop.number(), 9);
        assert_eq!(Step::PullRequest.number(), 19);
        for (i, s) in Step::ALL.iter().enumerate() {
            assert_eq!(usize::from(s.number()), i + 1);
        }
    }

    #[test]
    fn phases_match_the_kanban_columns() {
        assert_eq!(Step::UpdateBrain.phase(), Phase::Request);
        assert_eq!(Step::ApprovePrd.phase(), Phase::Planning);
        assert_eq!(Step::LogChanges.phase(), Phase::Building);
        assert_eq!(Step::Feedback.phase(), Phase::Testing);
        assert_eq!(Step::PullRequest.phase(), Phase::Ship);
        assert_eq!(Step::Done.phase(), Phase::Done);
    }

    #[test]
    fn full_suite_is_skipped_unless_enabled() {
        let off = FlowOptions::default();
        let on = FlowOptions {
            full_suite: true,
            ..off
        };
        assert_eq!(next_step(Step::Develop, off), Some(Step::LogChanges));
        assert_eq!(next_step(Step::Develop, on), Some(Step::FullSuite));
    }

    #[test]
    fn feedback_loops_back_to_develop() {
        check_transition(Step::Feedback, Step::Develop, FlowOptions::default()).unwrap();
    }

    #[test]
    fn building_needs_an_approved_prd() {
        let err = check_transition(Step::GrillPrd, Step::CreateBranch, FlowOptions::default())
            .unwrap_err()
            .to_string();
        assert!(err.contains("approved"), "{err}");
    }

    #[test]
    fn cannot_jump_back_without_feedback() {
        assert!(check_transition(Step::Commit, Step::Develop, FlowOptions::default()).is_err());
    }

    #[test]
    fn db_lock_steps() {
        assert!(Step::HumanTest.needs_db_lock());
        assert!(!Step::Develop.needs_db_lock());
    }
}
