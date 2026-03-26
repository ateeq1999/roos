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

/// Whether retries use a constant delay or exponential back-off.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RetryStrategy {
    /// Every retry waits exactly `retry_delay_seconds`.
    Fixed,
    /// Each retry doubles the previous wait: delay * 2^(attempt-1).
    Exponential,
}

/// Configurable retry behaviour attached to a scheduled task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts after the first failure (default: 3).
    pub max_retries: u32,
    /// Base delay in seconds between retries (default: 60).
    pub retry_delay_seconds: u64,
    /// Back-off strategy (default: `Exponential`).
    pub strategy: RetryStrategy,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_delay_seconds: 60,
            strategy: RetryStrategy::Exponential,
        }
    }
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
    /// Retry policy governing failure handling.
    pub retry_policy: RetryPolicy,
    /// Number of consecutive failures recorded so far.
    pub retry_count: u32,
}
