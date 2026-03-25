use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use roos_core::memory::{ConversationHistory, ConversationMessage, Memory, MemoryError};
use uuid::Uuid;

/// Ephemeral, in-process memory store backed by a `HashMap` behind an
/// `RwLock`.
///
/// Data is lost when the process exits. This is the default backend for
/// development and testing; production deployments should use `SledMemory`
/// (Task 26) or `QdrantMemory` (Task 37).
pub struct InMemoryStore {
    inner: RwLock<HashMap<Uuid, ConversationHistory>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for InMemoryStore {
    async fn store(&self, history: &ConversationHistory) -> Result<(), MemoryError> {
        self.inner
            .write()
            .map_err(|e| MemoryError::BackendError {
                source: e.to_string().into(),
            })?
            .insert(history.run_id, history.clone());
        Ok(())
    }

    async fn load(&self, run_id: Uuid) -> Result<Option<ConversationHistory>, MemoryError> {
        let guard = self.inner.read().map_err(|e| MemoryError::BackendError {
            source: e.to_string().into(),
        })?;
        Ok(guard.get(&run_id).cloned())
    }

    async fn append(&self, run_id: Uuid, message: ConversationMessage) -> Result<(), MemoryError> {
        let mut guard = self.inner.write().map_err(|e| MemoryError::BackendError {
            source: e.to_string().into(),
        })?;
        guard
            .entry(run_id)
            .or_insert_with(|| ConversationHistory::new(run_id))
            .push(message);
        Ok(())
    }

    async fn clear(&self, run_id: Uuid) -> Result<(), MemoryError> {
        self.inner
            .write()
            .map_err(|e| MemoryError::BackendError {
                source: e.to_string().into(),
            })?
            .remove(&run_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use roos_core::memory::ConversationMessage;

    use super::*;

    fn store() -> InMemoryStore {
        InMemoryStore::new()
    }

    #[tokio::test]
    async fn load_missing_returns_none() {
        let s = store();
        assert!(s.load(Uuid::new_v4()).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn store_then_load_roundtrip() {
        let s = store();
        let id = Uuid::new_v4();
        let mut h = ConversationHistory::new(id);
        h.push(ConversationMessage::user("hello"));
        s.store(&h).await.unwrap();
        let loaded = s.load(id).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn append_creates_history_if_absent() {
        let s = store();
        let id = Uuid::new_v4();
        s.append(id, ConversationMessage::assistant("hi"))
            .await
            .unwrap();
        let h = s.load(id).await.unwrap().unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h.messages[0].role, "assistant");
    }

    #[tokio::test]
    async fn append_adds_to_existing_history() {
        let s = store();
        let id = Uuid::new_v4();
        s.append(id, ConversationMessage::user("a")).await.unwrap();
        s.append(id, ConversationMessage::assistant("b"))
            .await
            .unwrap();
        s.append(id, ConversationMessage::user("c")).await.unwrap();
        assert_eq!(s.load(id).await.unwrap().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn clear_removes_history() {
        let s = store();
        let id = Uuid::new_v4();
        s.append(id, ConversationMessage::user("x")).await.unwrap();
        s.clear(id).await.unwrap();
        assert!(s.load(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn clear_nonexistent_is_ok() {
        let s = store();
        assert!(s.clear(Uuid::new_v4()).await.is_ok());
    }

    #[tokio::test]
    async fn default_is_empty() {
        let s = InMemoryStore::default();
        assert!(s.load(Uuid::new_v4()).await.unwrap().is_none());
    }
}
