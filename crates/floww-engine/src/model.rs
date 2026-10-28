//! Data the engine keeps per PRD. Saved as `.floww/runs/<id>.json`.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use floww_core::pipeline::{Phase, Step};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrdType {
    Bug,
    Improve,
    Feature,
}

impl PrdType {
    /// Odoo commit tag.
    pub fn commit_tag(self) -> &'static str {
        match self {
            PrdType::Bug => "[FIX]",
            PrdType::Improve => "[IMP]",
            PrdType::Feature => "[ADD]",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// The AI or the app is working.
    Ai,
    /// Waiting for a person.
    Needs,
    /// Waiting for the shared database.
    Wait,
    /// Pull request open.
    Review,
    Done,
    /// Stopped by an error; the action says what to do.
    Failed,
    /// Paused by a person.
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryState {
    Done,
    Run,
    Fail,
    Info,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub at: DateTime<Utc>,
    pub state: EntryState,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrillOption {
    pub label: String,
    #[serde(default)]
    pub why: String,
    #[serde(default)]
    pub recommended: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrillQuestion {
    pub topic: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain: Option<String>,
    pub options: Vec<GrillOption>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrillAnswer {
    pub question: GrillQuestion,
    pub answer: String,
    /// The person typed their own answer instead of picking an option.
    pub own: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrillState {
    #[default]
    Asking,
    Finished,
    Writing,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Grill {
    pub session_id: Option<String>,
    pub current: Option<GrillQuestion>,
    pub answers: Vec<GrillAnswer>,
    pub state: GrillState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Criterion {
    pub given: String,
    pub when: String,
    pub then: String,
    #[serde(default)]
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestCase {
    pub kind: String,
    pub title: String,
    pub steps: String,
    #[serde(default)]
    pub data: String,
    pub expected: String,
    #[serde(default)]
    pub result: Option<TestResult>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestResult {
    Pass,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OdooState {
    #[default]
    Off,
    Starting,
    On,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub why: String,
    #[serde(default)]
    pub enforced_in: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Context {
    pub summary: String,
    pub modules: Vec<String>,
    pub files: Vec<String>,
    pub brain_notes: Vec<String>,
    pub db_facts: Vec<String>,
    pub similar: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Ship {
    pub stage: u8,
    pub message: String,
    pub commit: Option<String>,
    pub pr_url: Option<String>,
    pub pr_number: Option<u64>,
    pub pr_title: String,
    pub pr_body: String,
    pub skipped_pr: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prd {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: PrdType,
    pub reporter: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub step: Step,
    pub status: Status,
    /// What the person should do when status is `needs` or `failed`.
    pub action: Option<String>,
    /// One-line status for cards.
    pub sub: String,
    pub module: Option<String>,
    pub modules: Vec<String>,
    pub timeline: Vec<Entry>,
    pub context: Option<Context>,
    pub grill: Option<Grill>,
    pub prd_markdown: Option<String>,
    pub criteria: Vec<Criterion>,
    pub rules: Vec<Rule>,
    pub branch: Option<String>,
    pub worktree: Option<PathBuf>,
    /// Commit the worktree started from (merge base for the final commit).
    pub base_commit: Option<String>,
    pub iteration: u32,
    pub tests: Vec<TestCase>,
    pub odoo: OdooState,
    pub feedback_rounds: u32,
    pub feedback: Vec<String>,
    pub ship: Option<Ship>,
    pub cost_usd: f64,
    pub tokens_in: u64,
    pub tokens_out: u64,
}

impl Prd {
    pub fn new(
        id: String,
        title: String,
        description: String,
        kind: PrdType,
        reporter: String,
        author: String,
    ) -> Self {
        let mut prd = Self {
            id,
            title,
            description,
            kind,
            reporter,
            author,
            created_at: Utc::now(),
            step: Step::NewRequest,
            status: Status::Ai,
            action: None,
            sub: "Request logged".into(),
            module: None,
            modules: Vec::new(),
            timeline: Vec::new(),
            context: None,
            grill: None,
            prd_markdown: None,
            criteria: Vec::new(),
            rules: Vec::new(),
            branch: None,
            worktree: None,
            base_commit: None,
            iteration: 0,
            tests: Vec::new(),
            odoo: OdooState::Off,
            feedback_rounds: 0,
            feedback: Vec::new(),
            ship: None,
            cost_usd: 0.0,
            tokens_in: 0,
            tokens_out: 0,
        };
        prd.log(EntryState::Done, "Request logged", None);
        prd
    }

    pub fn phase(&self) -> Phase {
        self.step.phase()
    }

    /// Add a timeline entry. A new entry closes any running one.
    pub fn log(&mut self, state: EntryState, text: impl Into<String>, cmd: Option<String>) {
        for e in &mut self.timeline {
            if e.state == EntryState::Run {
                e.state = EntryState::Done;
            }
        }
        self.timeline.push(Entry {
            at: Utc::now(),
            state,
            text: text.into(),
            cmd,
        });
    }

    pub fn working(&mut self, step: Step, sub: impl Into<String>) {
        self.step = step;
        self.status = Status::Ai;
        self.action = None;
        self.sub = sub.into();
    }

    pub fn needs(&mut self, step: Step, action: impl Into<String>, sub: impl Into<String>) {
        self.step = step;
        self.status = Status::Needs;
        self.action = Some(action.into());
        self.sub = sub.into();
    }

    pub fn fail(&mut self, action: impl Into<String>, why: impl Into<String>) {
        let why = why.into();
        self.status = Status::Failed;
        self.action = Some(action.into());
        self.sub = why.clone();
        self.log(EntryState::Fail, why, None);
    }

    pub fn slug(&self) -> String {
        slug(&self.title)
    }
}

/// First three words, lowercase, dash separated.
pub fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(3)
        .collect::<Vec<_>>()
        .join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_takes_three_words() {
        assert_eq!(
            slug("Invoice dobel saat pengiriman parsial"),
            "invoice-dobel-saat"
        );
        assert_eq!(slug("PO > Rp 50 juta!"), "po-rp-50");
    }

    #[test]
    fn new_log_closes_running_entry() {
        let mut p = Prd::new(
            "SP-001".into(),
            "t".into(),
            "".into(),
            PrdType::Bug,
            "".into(),
            "SP".into(),
        );
        p.log(EntryState::Run, "a", None);
        p.log(EntryState::Done, "b", None);
        assert!(p.timeline.iter().all(|e| e.state != EntryState::Run));
    }
}
