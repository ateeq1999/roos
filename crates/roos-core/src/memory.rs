use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── ConversationMessage ──────────────────────────────────────────────────────

/// A single message in a conversation, decoupled from any specific provider.
///
/// The `role` field follows the OpenAI/Anthropic convention: `"user"`,
/// `"assistant"`, or `"system"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
}

impl ConversationMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".into(),
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: content.into(),
        }
    }
}

// ── ConversationHistory ──────────────────────────────────────────────────────

/// The full message history for one agent run, keyed by `run_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationHistory {
    /// Run identifier — matches [`AgentInput::run_id`](crate::AgentInput).
    pub run_id: Uuid,
    /// Ordered list of messages, oldest first.
    pub messages: Vec<ConversationMessage>,
}

impl ConversationHistory {
    pub fn new(run_id: Uuid) -> Self {
        Self {
            run_id,
            messages: Vec::new(),
        }
    }

    /// Append a single message to the history.
    pub fn push(&mut self, message: ConversationMessage) {
        self.messages.push(message);
    }

    /// Total number of messages stored.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

// ── MemoryError ──────────────────────────────────────────────────────────────

/// Errors returned by [`Memory`] implementations.
#[derive(Debug)]
pub enum MemoryError {
    /// No history exists for the given `run_id`.
    NotFound { run_id: Uuid },
    /// The history for this `run_id` has passed its TTL and been evicted.
    Expired { run_id: Uuid },
    /// The backend (Sled, Qdrant, etc.) returned an error.
    BackendError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Serializing or deserializing history failed.
    SerializationError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl fmt::Display for MemoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { run_id } => write!(f, "no memory found for run {run_id}"),
            Self::Expired { run_id } => write!(f, "memory for run {run_id} has expired"),
            Self::BackendError { source } => write!(f, "memory backend error: {source}"),
            Self::SerializationError { source } => {
                write!(f, "memory serialization error: {source}")
            }
        }
    }
}

impl std::error::Error for MemoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BackendError { source } | Self::SerializationError { source } => {
                Some(source.as_ref())
            }
            _ => None,
        }
    }
}

// ── Memory trait ─────────────────────────────────────────────────────────────

/// Persistent or ephemeral storage for agent conversation history.
///
/// Backends implement this trait: [`InMemoryStore`](crate) (Task 8),
/// `SledMemory` (Task 26), and `QdrantMemory` (Task 37).
#[async_trait]
pub trait Memory: Send + Sync {
    /// Persist (overwrite) the full history for a run.
    async fn store(&self, history: &ConversationHistory) -> Result<(), MemoryError>;

    /// Load the history for a run. Returns `Ok(None)` if not found.
    async fn load(&self, run_id: Uuid) -> Result<Option<ConversationHistory>, MemoryError>;

    /// Append a single message to an existing history.
    ///
    /// Creates a new history entry if none exists for `run_id`.
    async fn append(&self, run_id: Uuid, message: ConversationMessage) -> Result<(), MemoryError>;

    /// Delete the history for a run.
    async fn clear(&self, run_id: Uuid) -> Result<(), MemoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ConversationMessage helpers ──────────────────────────────────────────

    #[test]
    fn message_constructors_set_roles() {
        assert_eq!(ConversationMessage::user("hi").role, "user");
        assert_eq!(ConversationMessage::assistant("ok").role, "assistant");
        assert_eq!(ConversationMessage::system("be helpful").role, "system");
    }

    #[test]
    fn message_roundtrips_json() {
        let m = ConversationMessage::user("hello");
        let json = serde_json::to_string(&m).unwrap();
        let back: ConversationMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(m, back);
    }

    // ── ConversationHistory ──────────────────────────────────────────────────

    #[test]
    fn history_push_and_len() {
        let id = Uuid::new_v4();
        let mut h = ConversationHistory::new(id);
        assert!(h.is_empty());
        h.push(ConversationMessage::user("hello"));
        h.push(ConversationMessage::assistant("hi"));
        assert_eq!(h.len(), 2);
        assert!(!h.is_empty());
    }

    // ── MemoryError display ──────────────────────────────────────────────────

    #[test]
    fn not_found_display() {
        let id = Uuid::nil();
        let e = MemoryError::NotFound { run_id: id };
        assert!(e.to_string().contains("no memory found for run"));
    }

    #[test]
    fn expired_display() {
        let id = Uuid::nil();
        let e = MemoryError::Expired { run_id: id };
        assert!(e.to_string().contains("has expired"));
    }

    #[test]
    fn backend_error_display() {
        let src: Box<dyn std::error::Error + Send + Sync> = "disk full".into();
        let e = MemoryError::BackendError { source: src };
        assert_eq!(e.to_string(), "memory backend error: disk full");
    }

    #[test]
    fn serialization_error_source_is_some() {
        let src: Box<dyn std::error::Error + Send + Sync> = "bad bytes".into();
        let e = MemoryError::SerializationError { source: src };
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn not_found_source_is_none() {
        let e = MemoryError::NotFound {
            run_id: Uuid::nil(),
        };
        assert!(std::error::Error::source(&e).is_none());
    }

    // ── Object-safety check ──────────────────────────────────────────────────

    struct StubMemory;

    #[async_trait]
    impl Memory for StubMemory {
        async fn store(&self, _: &ConversationHistory) -> Result<(), MemoryError> {
            Ok(())
        }
        async fn load(&self, _: Uuid) -> Result<Option<ConversationHistory>, MemoryError> {
            Ok(None)
        }
        async fn append(&self, _: Uuid, _: ConversationMessage) -> Result<(), MemoryError> {
            Ok(())
        }
        async fn clear(&self, _: Uuid) -> Result<(), MemoryError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn memory_is_object_safe() {
        let mem: Box<dyn Memory> = Box::new(StubMemory);
        let id = Uuid::new_v4();
        assert!(mem.load(id).await.unwrap().is_none());
    }
}
