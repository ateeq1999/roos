use std::fmt;

use uuid::Uuid;

#[derive(Debug)]
pub enum SchedulerError {
    BackendError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    InvalidSchedule {
        reason: String,
    },
    TaskNotFound {
        id: Uuid,
    },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BackendError { source } => write!(f, "scheduler backend error: {source}"),
            Self::InvalidSchedule { reason } => write!(f, "invalid cron schedule: {reason}"),
            Self::TaskNotFound { id } => write!(f, "task not found: {id}"),
        }
    }
}

impl std::error::Error for SchedulerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::BackendError { source } = self {
            Some(source.as_ref())
        } else {
            None
        }
    }
}
