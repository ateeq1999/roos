use std::path::Path;

use roos_core::RoosConfig;

/// Agent info extracted from `roos.toml` for display.
#[derive(Debug)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub provider: String,
    pub model: String,
}

/// Parse `config_path` and return a list of agents.
///
/// Currently `roos.toml` describes exactly one agent; this returns a
/// single-element list so the display layer is trivially extensible.
pub fn load_agents(config_path: &str) -> anyhow::Result<Vec<AgentInfo>> {
    let cfg = RoosConfig::from_file(Path::new(config_path))
        .map_err(|e| anyhow::anyhow!("Failed to load '{}': {e}", config_path))?;

    Ok(vec![AgentInfo {
        name: cfg.agent.name,
        description: cfg.agent.description,
        provider: cfg.provider.provider_type,
        model: cfg.provider.model,
    }])
}

pub fn run(config_path: &str) -> anyhow::Result<()> {
    let agents = load_agents(config_path)?;

    println!(
        "{:<30} {:<40} {:<12} MODEL",
        "NAME", "DESCRIPTION", "PROVIDER"
    );
    println!("{}", "-".repeat(90));
    for a in &agents {
        println!(
            "{:<30} {:<40} {:<12} {}",
            a.name, a.description, a.provider, a.model
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_config(tmp: &TempDir, content: &str) -> String {
        let path = tmp.path().join("roos.toml");
        fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    }

    const SAMPLE_TOML: &str = r#"
[agent]
name = "code-helper"
description = "Helps with code"

[provider]
type = "openai"
model = "gpt-4o"
"#;

    #[test]
    fn loads_agent_info() {
        let tmp = TempDir::new().unwrap();
        let path = write_config(&tmp, SAMPLE_TOML);
        let agents = load_agents(&path).unwrap();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "code-helper");
        assert_eq!(agents[0].provider, "openai");
        assert_eq!(agents[0].model, "gpt-4o");
    }

    #[test]
    fn missing_config_returns_error() {
        let err = load_agents("/nonexistent/path/roos.toml").unwrap_err();
        assert!(err.to_string().contains("Failed to load"));
    }
}
