use std::fmt;

/// The phase an agent run is currently in.
///
/// Transitions follow the Reasoning → Action → Observation cycle defined in
/// ROOS-ORCH-001. Only valid transitions are permitted; attempting an invalid
/// one returns [`TransitionError`].
///
/// ```text
///  Idle ──start──► Reasoning ──tool_call──► CallingTool ──tool_done──► Observing
///                     ▲                                                     │
///                     └─────────────────── continue ────────────────────────┘
///                     │
///              finish (EndTurn)
///                     │
///                     ▼
///                  Finished
///
///  Any state ──fail──► Failed
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// Run has not started yet.
    Idle,
    /// Waiting for the LLM to produce a response (step N).
    Reasoning { step: usize },
    /// Executing a tool invocation requested by the LLM (step N).
    CallingTool { tool_name: String, step: usize },
    /// Tool returned; preparing next reasoning cycle (step N).
    Observing { step: usize },
    /// Run completed — LLM produced a final response.
    Finished,
    /// Run aborted (error or `max_steps` exceeded).
    Failed,
}

impl AgentState {
    /// Returns `true` for terminal states (`Finished` or `Failed`).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Finished | Self::Failed)
    }

    /// The current step number, or `0` for `Idle` / terminal states.
    pub fn step(&self) -> usize {
        match self {
            Self::Reasoning { step }
            | Self::CallingTool { step, .. }
            | Self::Observing { step } => *step,
            _ => 0,
        }
    }

    /// `Idle → Reasoning { step: 1 }`.
    pub fn start(self) -> Result<Self, TransitionError> {
        match self {
            Self::Idle => Ok(Self::Reasoning { step: 1 }),
            other => Err(TransitionError::invalid("start", &other)),
        }
    }

    /// `Reasoning { step } → CallingTool { tool_name, step }`.
    pub fn call_tool(self, tool_name: impl Into<String>) -> Result<Self, TransitionError> {
        match self {
            Self::Reasoning { step } => Ok(Self::CallingTool {
                tool_name: tool_name.into(),
                step,
            }),
            other => Err(TransitionError::invalid("call_tool", &other)),
        }
    }

    /// `CallingTool { step } → Observing { step }`.
    pub fn tool_done(self) -> Result<Self, TransitionError> {
        match self {
            Self::CallingTool { step, .. } => Ok(Self::Observing { step }),
            other => Err(TransitionError::invalid("tool_done", &other)),
        }
    }

    /// `Observing { step } → Reasoning { step: step + 1 }`.
    pub fn continue_reasoning(self) -> Result<Self, TransitionError> {
        match self {
            Self::Observing { step } => Ok(Self::Reasoning { step: step + 1 }),
            other => Err(TransitionError::invalid("continue_reasoning", &other)),
        }
    }

    /// `Reasoning → Finished` (LLM returned `StopReason::EndTurn`).
    pub fn finish(self) -> Result<Self, TransitionError> {
        match self {
            Self::Reasoning { .. } => Ok(Self::Finished),
            other => Err(TransitionError::invalid("finish", &other)),
        }
    }

    /// Any state → `Failed`.
    pub fn fail(self) -> Self {
        Self::Failed
    }
}

impl fmt::Display for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Reasoning { step } => write!(f, "Reasoning(step={step})"),
            Self::CallingTool { tool_name, step } => {
                write!(f, "CallingTool(tool={tool_name}, step={step})")
            }
            Self::Observing { step } => write!(f, "Observing(step={step})"),
            Self::Finished => write!(f, "Finished"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

// ── TransitionError ───────────────────────────────────────────────────────────

/// Returned when an invalid state transition is attempted.
#[derive(Debug)]
pub struct TransitionError {
    pub event: String,
    pub from: String,
}

impl TransitionError {
    fn invalid(event: &str, from: &AgentState) -> Self {
        Self {
            event: event.to_owned(),
            from: from.to_string(),
        }
    }
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid transition `{}` from state `{}`",
            self.event, self.from
        )
    }
}

impl std::error::Error for TransitionError {}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Happy-path transitions ────────────────────────────────────────────────

    #[test]
    fn full_cycle_no_tools() {
        let s = AgentState::Idle;
        let s = s.start().unwrap();
        assert_eq!(s, AgentState::Reasoning { step: 1 });
        let s = s.finish().unwrap();
        assert_eq!(s, AgentState::Finished);
        assert!(s.is_terminal());
    }

    #[test]
    fn cycle_with_one_tool_call() {
        let s = AgentState::Idle.start().unwrap();
        let s = s.call_tool("read_file").unwrap();
        assert_eq!(
            s,
            AgentState::CallingTool {
                tool_name: "read_file".into(),
                step: 1,
            }
        );
        let s = s.tool_done().unwrap();
        assert_eq!(s, AgentState::Observing { step: 1 });
        let s = s.continue_reasoning().unwrap();
        assert_eq!(s, AgentState::Reasoning { step: 2 });
        let s = s.finish().unwrap();
        assert_eq!(s, AgentState::Finished);
    }

    #[test]
    fn step_increments_each_cycle() {
        let mut s = AgentState::Idle.start().unwrap();
        for expected in 1..=3usize {
            assert_eq!(s.step(), expected);
            s = s
                .call_tool("t")
                .unwrap()
                .tool_done()
                .unwrap()
                .continue_reasoning()
                .unwrap();
        }
    }

    #[test]
    fn fail_from_any_state() {
        assert_eq!(AgentState::Idle.fail(), AgentState::Failed);
        assert_eq!(AgentState::Reasoning { step: 1 }.fail(), AgentState::Failed);
        assert_eq!(AgentState::Finished.fail(), AgentState::Failed);
    }

    // ── Invalid transitions ───────────────────────────────────────────────────

    #[test]
    fn start_from_non_idle_fails() {
        let err = AgentState::Reasoning { step: 1 }.start().unwrap_err();
        assert!(err.to_string().contains("start"));
    }

    #[test]
    fn finish_from_idle_fails() {
        assert!(AgentState::Idle.finish().is_err());
    }

    #[test]
    fn tool_done_from_reasoning_fails() {
        assert!(AgentState::Reasoning { step: 1 }.tool_done().is_err());
    }

    #[test]
    fn continue_from_reasoning_fails() {
        assert!(AgentState::Reasoning { step: 1 }
            .continue_reasoning()
            .is_err());
    }

    // ── Display / terminal ────────────────────────────────────────────────────

    #[test]
    fn display_variants() {
        assert_eq!(AgentState::Idle.to_string(), "Idle");
        assert_eq!(
            AgentState::Reasoning { step: 2 }.to_string(),
            "Reasoning(step=2)"
        );
        assert_eq!(AgentState::Finished.to_string(), "Finished");
        assert_eq!(AgentState::Failed.to_string(), "Failed");
    }

    #[test]
    fn is_terminal_only_for_finished_and_failed() {
        assert!(!AgentState::Idle.is_terminal());
        assert!(!AgentState::Reasoning { step: 1 }.is_terminal());
        assert!(AgentState::Finished.is_terminal());
        assert!(AgentState::Failed.is_terminal());
    }
}
