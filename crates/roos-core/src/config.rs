use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

// ── Error ────────────────────────────────────────────────────────────────────

/// Errors that can occur while loading or parsing `roos.toml`.
#[derive(Debug)]
pub enum ConfigError {
    /// The config file could not be read from disk.
    Io(std::io::Error),
    /// The TOML was syntactically or semantically invalid.
    Parse(toml::de::Error),
    /// A `${VAR}` placeholder references an env var that is not set.
    MissingEnvVar(String),
    /// A `${...}` placeholder is malformed (no closing `}`).
    MalformedPlaceholder { at: usize },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "config I/O error: {e}"),
            Self::Parse(e) => write!(f, "config parse error: {e}"),
            Self::MissingEnvVar(v) => write!(f, "env var '${{{v}}}' is not set"),
            Self::MalformedPlaceholder { at } => {
                write!(f, "malformed '${{...}}' placeholder at byte {at}")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Parse(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(e: toml::de::Error) -> Self {
        Self::Parse(e)
    }
}

// ── Config structs ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub name: String,
    pub description: String,
    pub max_steps: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Provider type: `"anthropic"`, `"openai"`, or `"ollama"`.
    #[serde(rename = "type")]
    pub provider_type: String,
    pub model: String,
    /// API key — typically set via `${ANTHROPIC_API_KEY}` interpolation.
    pub api_key: Option<String>,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Memory backend: `"in-memory"`, `"sled"`, or `"qdrant"`.
    pub backend: String,
}

/// Root configuration loaded from `roos.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoosConfig {
    pub agent: AgentConfig,
    pub provider: ProviderConfig,
    pub memory: Option<MemoryConfig>,
}

impl RoosConfig {
    /// Load and parse `roos.toml` from `path`, interpolating `${VAR}` env vars.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path)?;
        Self::parse(&raw)
    }

    /// Parse a TOML string, interpolating `${VAR}` env vars in all values.
    pub fn parse(toml: &str) -> Result<Self, ConfigError> {
        let interpolated = interpolate_env_vars(toml)?;
        Ok(toml::from_str(&interpolated)?)
    }
}

// ── Env var interpolation ─────────────────────────────────────────────────────

/// Replace every `${VAR_NAME}` occurrence in `s` with the value of the
/// named environment variable.
///
/// Returns [`ConfigError::MissingEnvVar`] if the variable is not set, or
/// [`ConfigError::MalformedPlaceholder`] if `${` has no matching `}`.
fn interpolate_env_vars(s: &str) -> Result<String, ConfigError> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;

    while let Some(start) = rest.find("${") {
        // Everything before the placeholder.
        out.push_str(&rest[..start]);
        let after_dollar = &rest[start + 2..];
        let close = after_dollar
            .find('}')
            .ok_or(ConfigError::MalformedPlaceholder {
                at: s.len() - rest.len() + start,
            })?;
        let var_name = &after_dollar[..close];
        let value =
            std::env::var(var_name).map_err(|_| ConfigError::MissingEnvVar(var_name.to_owned()))?;
        out.push_str(&value);
        // Advance past the closing `}`.
        rest = &after_dollar[close + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL_TOML: &str = r#"
[agent]
name = "test-agent"
description = "A test agent"

[provider]
type = "anthropic"
model = "claude-sonnet-4-6"
"#;

    #[test]
    fn parse_minimal_config() {
        let cfg = RoosConfig::parse(MINIMAL_TOML).unwrap();
        assert_eq!(cfg.agent.name, "test-agent");
        assert_eq!(cfg.provider.provider_type, "anthropic");
        assert_eq!(cfg.provider.model, "claude-sonnet-4-6");
        assert!(cfg.memory.is_none());
    }

    #[test]
    fn parse_with_memory_section() {
        let toml = format!("{MINIMAL_TOML}\n[memory]\nbackend = \"sled\"\n");
        let cfg = RoosConfig::parse(&toml).unwrap();
        assert_eq!(cfg.memory.unwrap().backend, "sled");
    }

    #[test]
    fn env_var_interpolation() {
        std::env::set_var("ROOS_TEST_KEY", "secret-value");
        let toml = r#"
[agent]
name = "a"
description = "b"

[provider]
type = "anthropic"
model = "m"
api_key = "${ROOS_TEST_KEY}"
"#;
        let cfg = RoosConfig::parse(toml).unwrap();
        assert_eq!(cfg.provider.api_key.as_deref(), Some("secret-value"));
        std::env::remove_var("ROOS_TEST_KEY");
    }

    #[test]
    fn missing_env_var_returns_error() {
        std::env::remove_var("ROOS_DEFINITELY_MISSING");
        let toml = r#"
[agent]
name = "a"
description = "b"

[provider]
type = "x"
model = "m"
api_key = "${ROOS_DEFINITELY_MISSING}"
"#;
        let err = RoosConfig::parse(toml).unwrap_err();
        assert!(matches!(err, ConfigError::MissingEnvVar(_)));
    }

    #[test]
    fn malformed_placeholder_returns_error() {
        let err = interpolate_env_vars("value = \"${UNCLOSED\"").unwrap_err();
        assert!(matches!(err, ConfigError::MalformedPlaceholder { .. }));
    }

    #[test]
    fn no_placeholders_passes_through() {
        let result = interpolate_env_vars("plain = \"value\"").unwrap();
        assert_eq!(result, "plain = \"value\"");
    }
}
