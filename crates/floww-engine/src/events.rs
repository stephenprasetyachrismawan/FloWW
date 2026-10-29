//! Events pushed to the UI (desktop and web) over server-sent events.

use serde::Serialize;

use crate::model::Prd;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    PrdChanged {
        workspace: String,
        prd: Box<Prd>,
    },
    WorkspacesChanged,
    PrdDeleted {
        workspace: String,
        id: String,
    },
    LockChanged {
        workspace: String,
    },
    BrainChanged {
        workspace: String,
    },
    Notify {
        title: String,
        body: String,
        kind: NotifyKind,
    },
    /// Progress of a floww-web self-update.
    Update(crate::update::Progress),
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyKind {
    You,
    Ok,
    Bad,
    Info,
}
