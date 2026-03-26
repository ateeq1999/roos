use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use tokio::sync::{broadcast, mpsc, oneshot};

const AGENT_QUEUE_CAPACITY: usize = 32;
const BROADCAST_CAPACITY: usize = 16;

// ── Message types ─────────────────────────────────────────────────────────────

/// Task payload sent point-to-point from one agent to another.
pub struct BusMessage {
    /// The input text for the target agent.
    pub input: String,
    /// One-shot channel on which the target agent MUST send its reply.
    pub reply_tx: oneshot::Sender<String>,
}

/// Event broadcast to all subscribers of a named topic.
#[derive(Debug, Clone)]
pub struct BusEvent {
    pub topic: String,
    pub payload: String,
}

// ── Error ─────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum BusError {
    /// No agent with this name is currently registered on the bus.
    AgentNotFound { name: String },
    /// The target agent's queue is full or its receiver has been dropped.
    SendError,
    /// The agent dropped the reply channel without sending a response.
    RecvError,
}

impl std::fmt::Display for BusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AgentNotFound { name } => write!(f, "agent not found on bus: {name}"),
            Self::SendError => write!(f, "failed to deliver task to agent"),
            Self::RecvError => write!(f, "agent did not send a reply"),
        }
    }
}

impl std::error::Error for BusError {}

// ── RoosAgentBus ─────────────────────────────────────────────────────────────

/// Multi-agent communication bus backed by Tokio mpsc (direct tasks) and
/// broadcast (pub/sub events) channels.
///
/// Cheaply cloneable — all clones share the same underlying channel maps.
#[derive(Clone, Default)]
pub struct RoosAgentBus {
    agents: Arc<RwLock<HashMap<String, mpsc::Sender<BusMessage>>>>,
    topics: Arc<RwLock<HashMap<String, broadcast::Sender<BusEvent>>>>,
}

impl RoosAgentBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an agent by name and return its task receiver.
    ///
    /// The caller is responsible for driving the returned [`mpsc::Receiver`]
    /// — reading each [`BusMessage`] and sending a reply via `reply_tx`.
    pub fn register(&self, name: &str) -> mpsc::Receiver<BusMessage> {
        let (tx, rx) = mpsc::channel(AGENT_QUEUE_CAPACITY);
        self.agents.write().unwrap().insert(name.to_owned(), tx);
        rx
    }

    /// Send a task to a named agent and await its reply.
    pub async fn send(&self, agent: &str, input: &str) -> Result<String, BusError> {
        let tx = self
            .agents
            .read()
            .unwrap()
            .get(agent)
            .cloned()
            .ok_or_else(|| BusError::AgentNotFound {
                name: agent.to_owned(),
            })?;

        let (reply_tx, reply_rx) = oneshot::channel();
        tx.send(BusMessage {
            input: input.to_owned(),
            reply_tx,
        })
        .await
        .map_err(|_| BusError::SendError)?;

        reply_rx.await.map_err(|_| BusError::RecvError)
    }

    /// Subscribe to a named topic, creating the topic channel if needed.
    /// All current and future subscribers share the same broadcast channel.
    pub fn subscribe(&self, topic: &str) -> broadcast::Receiver<BusEvent> {
        let mut topics = self.topics.write().unwrap();
        let tx = topics
            .entry(topic.to_owned())
            .or_insert_with(|| broadcast::channel(BROADCAST_CAPACITY).0);
        tx.subscribe()
    }

    /// Publish an event to all current subscribers of `topic`.
    ///
    /// Returns the number of receivers that got the message, or `0` if the
    /// topic has no subscribers or has never been subscribed to.
    pub fn publish(&self, topic: &str, payload: &str) -> usize {
        let topics = self.topics.read().unwrap();
        if let Some(tx) = topics.get(topic) {
            tx.send(BusEvent {
                topic: topic.to_owned(),
                payload: payload.to_owned(),
            })
            .unwrap_or(0)
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    #[tokio::test]
    async fn send_task_and_receive_reply() {
        let bus = RoosAgentBus::new();
        let mut rx = bus.register("worker");

        // Simulate the worker agent: echo input back.
        tokio::spawn(async move {
            if let Some(msg) = rx.recv().await {
                let _ = msg.reply_tx.send(format!("echo: {}", msg.input));
            }
        });

        let reply = bus.send("worker", "hello").await.unwrap();
        assert_eq!(reply, "echo: hello");
    }

    #[tokio::test]
    async fn send_to_unknown_agent_errors() {
        let bus = RoosAgentBus::new();
        let err = bus.send("ghost", "hi").await.unwrap_err();
        assert!(matches!(err, BusError::AgentNotFound { .. }));
    }

    #[tokio::test]
    async fn subscribe_then_publish_delivers_event() {
        let bus = RoosAgentBus::new();
        let mut sub = bus.subscribe("alerts");
        bus.publish("alerts", "fire!");
        let ev = timeout(Duration::from_millis(200), sub.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ev.topic, "alerts");
        assert_eq!(ev.payload, "fire!");
    }

    #[tokio::test]
    async fn multiple_subscribers_all_receive_event() {
        let bus = RoosAgentBus::new();
        let mut sub1 = bus.subscribe("news");
        let mut sub2 = bus.subscribe("news");
        bus.publish("news", "update");
        let ev1 = timeout(Duration::from_millis(200), sub1.recv())
            .await
            .unwrap()
            .unwrap();
        let ev2 = timeout(Duration::from_millis(200), sub2.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ev1.payload, "update");
        assert_eq!(ev2.payload, "update");
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_returns_zero() {
        let bus = RoosAgentBus::new();
        assert_eq!(bus.publish("void", "data"), 0);
    }
}
