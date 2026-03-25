use std::fs;
use std::path::Path;

/// Scaffold a new ROOS project at `target_dir`, using `name` in the generated
/// `roos.toml`.  The directory must not already exist.
pub fn scaffold(target_dir: &Path, name: &str) -> anyhow::Result<()> {
    if target_dir.exists() {
        anyhow::bail!("directory '{}' already exists", target_dir.display());
    }

    fs::create_dir_all(target_dir.join("src"))?;
    fs::write(target_dir.join("roos.toml"), roos_toml(name))?;
    fs::write(target_dir.join("src").join("agent.rs"), AGENT_STUB)?;
    fs::write(target_dir.join(".gitignore"), "/target\n")?;

    println!("Created project '{name}'");
    println!("  {}/roos.toml", name);
    println!("  {}/src/agent.rs", name);
    println!("  {}/.gitignore", name);
    Ok(())
}

pub fn run(name: &str) -> anyhow::Result<()> {
    scaffold(Path::new(name), name)
}

fn roos_toml(name: &str) -> String {
    format!(
        r#"[agent]
name = "{name}"
description = "A ROOS agent"
max_steps = 10

[provider]
type = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${{ANTHROPIC_API_KEY}}"
"#
    )
}

const AGENT_STUB: &str = "// Agent implementation goes here.\n";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_expected_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("my-agent");
        scaffold(&dir, "my-agent").unwrap();

        assert!(dir.join("roos.toml").exists());
        assert!(dir.join("src").join("agent.rs").exists());
        assert!(dir.join(".gitignore").exists());
    }

    #[test]
    fn roos_toml_contains_project_name() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("hello");
        scaffold(&dir, "hello").unwrap();

        let toml = fs::read_to_string(dir.join("roos.toml")).unwrap();
        assert!(toml.contains("hello"));
        assert!(toml.contains("anthropic"));
        assert!(toml.contains("${ANTHROPIC_API_KEY}"));
    }

    #[test]
    fn fails_if_dir_exists() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("existing");
        fs::create_dir(&dir).unwrap();

        let err = scaffold(&dir, "existing").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn gitignore_contains_target() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("proj");
        scaffold(&dir, "proj").unwrap();

        let gi = fs::read_to_string(dir.join(".gitignore")).unwrap();
        assert!(gi.contains("target"));
    }
}
