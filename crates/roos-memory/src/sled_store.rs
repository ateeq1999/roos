use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use roos_core::memory::{ConversationHistory, ConversationMessage, Memory, MemoryError};

// ── On-disk entry ─────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    history: ConversationHistory,
    /// Optional Unix timestamp (seconds) after which the entry is considered expired.
    expires_at: Option<u64>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── SledMemory ────────────────────────────────────────────────────────────────

/// Persistent [`Memory`] backend backed by a [Sled](https://docs.rs/sled) embedded database.
///
/// Entries may be given a TTL (time-to-live) in seconds.  Expiry is enforced
/// lazily on [`load`](Memory::load) and [`append`](Memory::append) — no
/// background sweep is required.
///
/// # Example
///
/// ```no_run
/// # use roos_memory::SledMemory;
/// let memory = SledMemory::open("/tmp/roos-db").unwrap().with_ttl(3600);
/// ```
pub struct SledMemory {
    db: sled::Db,
    ttl_secs: Option<u64>,
}

impl SledMemory {
    /// Open (or create) the Sled database at `path`.
    pub fn open(path: &str) -> Result<Self, MemoryError> {
        let db = sled::open(path).map_err(|e| MemoryError::BackendError { source: e.into() })?;
        Ok(Self { db, ttl_secs: None })
    }

    /// Set a time-to-live for all subsequently stored entries.
    pub fn with_ttl(mut self, secs: u64) -> Self {
        self.ttl_secs = Some(secs);
        self
    }

    fn key(id: Uuid) -> [u8; 16] {
        *id.as_bytes()
    }

    fn make_entry(&self, history: &ConversationHistory) -> StoredEntry {
        StoredEntry {
            history: history.clone(),
            expires_at: self.ttl_secs.map(|s| now_secs() + s),
        }
    }

    fn decode(bytes: &[u8]) -> Result<StoredEntry, MemoryError> {
        serde_json::from_slice(bytes)
            .map_err(|e| MemoryError::SerializationError { source: e.into() })
    }

    fn encode(entry: &StoredEntry) -> Result<Vec<u8>, MemoryError> {
        serde_json::to_vec(entry).map_err(|e| MemoryError::SerializationError { source: e.into() })
    }
}

#[async_trait]
impl Memory for SledMemory {
    async fn store(&self, history: &ConversationHistory) -> Result<(), MemoryError> {
        let bytes = Self::encode(&self.make_entry(history))?;
        self.db
            .insert(Self::key(history.run_id), bytes)
            .map_err(|e| MemoryError::BackendError { source: e.into() })?;
        Ok(())
    }

    async fn load(&self, id: Uuid) -> Result<Option<ConversationHistory>, MemoryError> {
        let raw = self
            .db
            .get(Self::key(id))
            .map_err(|e| MemoryError::BackendError { source: e.into() })?;

        let bytes = match raw {
            None => return Ok(None),
            Some(b) => b,
        };

        let entry = Self::decode(&bytes)?;

        if let Some(exp) = entry.expires_at {
            if now_secs() > exp {
                // Lazy eviction: remove and surface as Expired.
                let _ = self.db.remove(Self::key(id));
                return Err(MemoryError::Expired { run_id: id });
            }
        }

        Ok(Some(entry.history))
    }

    async fn append(&self, id: Uuid, message: ConversationMessage) -> Result<(), MemoryError> {
        let mut history = match self.load(id).await {
            Ok(Some(h)) => h,
            Ok(None) => ConversationHistory::new(id),
            Err(MemoryError::Expired { .. }) => ConversationHistory::new(id),
            Err(e) => return Err(e),
        };
        history.push(message);
        self.store(&history).await
    }

    async fn clear(&self, id: Uuid) -> Result<(), MemoryError> {
        self.db
            .remove(Self::key(id))
            .map_err(|e| MemoryError::BackendError { source: e.into() })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roos_core::memory::ConversationMessage;
    use tempfile::TempDir;

    fn store(tmp: &TempDir) -> SledMemory {
        SledMemory::open(tmp.path().to_str().unwrap()).unwrap()
    }

    #[tokio::test]
    async fn store_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let mem = store(&tmp);
        let id = Uuid::new_v4();
        let mut h = ConversationHistory::new(id);
        h.push(ConversationMessage::user("hello"));
        mem.store(&h).await.unwrap();
        let loaded = mem.load(id).await.unwrap().unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn load_missing_returns_none() {
        let tmp = TempDir::new().unwrap();
        let mem = store(&tmp);
        let result = mem.load(Uuid::new_v4()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn append_creates_and_grows() {
        let tmp = TempDir::new().unwrap();
        let mem = store(&tmp);
        let id = Uuid::new_v4();
        mem.append(id, ConversationMessage::user("first"))
            .await
            .unwrap();
        mem.append(id, ConversationMessage::assistant("second"))
            .await
            .unwrap();
        let h = mem.load(id).await.unwrap().unwrap();
        assert_eq!(h.messages.len(), 2);
    }

    #[tokio::test]
    async fn clear_removes_entry() {
        let tmp = TempDir::new().unwrap();
        let mem = store(&tmp);
        let id = Uuid::new_v4();
        mem.append(id, ConversationMessage::user("hi"))
            .await
            .unwrap();
        mem.clear(id).await.unwrap();
        assert!(mem.load(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn expired_entry_returns_error() {
        let tmp = TempDir::new().unwrap();
        let mem = SledMemory::open(tmp.path().to_str().unwrap())
            .unwrap()
            .with_ttl(0); // TTL of 0 seconds — expires immediately
        let id = Uuid::new_v4();
        let mut h = ConversationHistory::new(id);
        h.push(ConversationMessage::user("test"));
        // Store with ttl=0 so expires_at == now.
        mem.store(&h).await.unwrap();
        // Sleep 1s so now > expires_at.
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let result = mem.load(id).await;
        assert!(matches!(result, Err(MemoryError::Expired { .. })));
    }
}
