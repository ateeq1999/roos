use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskKind {
    /// Repeating task driven by a cron expression.
    Cron { expr: String },
    /// Execute once at the specified time; does not reschedule.
    OneShot,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskState {
    Pending,
    Running,
    Completed,
    Failed { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub agent: String,
    pub kind: TaskKind,
    pub input: String,
    pub state: TaskState,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
}
