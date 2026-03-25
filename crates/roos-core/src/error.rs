use std::fmt;

/// Top-level error type returned by all agent operations.
///
/// Every variant implements [`std::error::Error`] and [`fmt::Display`].
/// No public API path panics — all failures surface as `Err(AgentError)`.
#[derive(Debug)]
pub enum AgentError {
    /// An LLM provider API call failed (network error, auth failure, 5xx, etc.).
    ProviderError(String),

    /// A tool execution failed.
    ToolError {
        /// Name of the tool that failed.
        name: String,
        /// Underlying cause.
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// The agent exceeded its configured `max_steps` limit.
    MaxStepsExceeded(usize),

    /// The conversation history exceeds the provider's context window.
    ContextWindowExceeded,

    /// The memory backend (Sled, Qdrant, etc.) returned an error.
    MemoryError(String),

    /// The harness configuration (`roos.toml` or code) is malformed.
    ConfigurationError(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProviderError(msg) => write!(f, "provider error: {msg}"),
            Self::ToolError { name, source } => {
                write!(f, "tool '{name}' failed: {source}")
            }
            Self::MaxStepsExceeded(n) => {
                write!(f, "agent exceeded maximum steps ({n})")
            }
            Self::ContextWindowExceeded => {
                write!(f, "context window exceeded")
            }
            Self::MemoryError(msg) => write!(f, "memory error: {msg}"),
            Self::ConfigurationError(msg) => write!(f, "configuration error: {msg}"),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ToolError { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_display() {
        let e = AgentError::ProviderError("timeout".into());
        assert_eq!(e.to_string(), "provider error: timeout");
    }

    #[test]
    fn tool_error_display() {
        let inner: Box<dyn std::error::Error + Send + Sync> = "file not found".into();
        let e = AgentError::ToolError {
            name: "read_file".into(),
            source: inner,
        };
        assert_eq!(e.to_string(), "tool 'read_file' failed: file not found");
    }

    #[test]
    fn max_steps_display() {
        let e = AgentError::MaxStepsExceeded(20);
        assert_eq!(e.to_string(), "agent exceeded maximum steps (20)");
    }

    #[test]
    fn context_window_display() {
        let e = AgentError::ContextWindowExceeded;
        assert_eq!(e.to_string(), "context window exceeded");
    }

    #[test]
    fn memory_error_display() {
        let e = AgentError::MemoryError("sled write failed".into());
        assert_eq!(e.to_string(), "memory error: sled write failed");
    }

    #[test]
    fn configuration_error_display() {
        let e = AgentError::ConfigurationError("missing api_key".into());
        assert_eq!(e.to_string(), "configuration error: missing api_key");
    }

    #[test]
    fn tool_error_source_is_some() {
        let inner: Box<dyn std::error::Error + Send + Sync> = "boom".into();
        let e = AgentError::ToolError {
            name: "shell".into(),
            source: inner,
        };
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn provider_error_source_is_none() {
        let e = AgentError::ProviderError("bad gateway".into());
        assert!(std::error::Error::source(&e).is_none());
    }
}
