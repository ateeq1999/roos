use std::path::Path;
use std::time::Duration;

use roos_core::RoosConfig;
use roos_scheduler::CronScheduler;
use roos_trigger::TriggerServer;
use tokio::sync::oneshot;

/// Format the TCP bind address for a given port.
pub(crate) fn bind_addr(port: u16) -> String {
    format!("0.0.0.0:{port}")
}

/// Derive the scheduler Sled DB path from the roos.toml path.
pub(crate) fn sched_db_path(config_path: &str) -> std::path::PathBuf {
    Path::new(config_path)
        .parent()
        .unwrap_or(Path::new("."))
        .join("roos-scheduler.db")
}

pub async fn run(port: u16, config_path: &str, daemonize: bool) -> anyhow::Result<()> {
    if daemonize {
        #[cfg(unix)]
        eprintln!("warning: --daemonize is not yet implemented, running in foreground");
        #[cfg(not(unix))]
        eprintln!("warning: --daemonize is not supported on Windows");
    }

    let _ = roos_observability::init_logging("info");

    let cfg = RoosConfig::from_file(Path::new(config_path))
        .map_err(|e| anyhow::anyhow!("Failed to load '{}': {e}", config_path))?;

    let sched_path = sched_db_path(config_path);
    let scheduler = CronScheduler::open(sched_path.to_str().unwrap())
        .map_err(|e| anyhow::anyhow!("Failed to open scheduler store: {e}"))?;

    let server = TriggerServer::new();
    server.state().register_agent(&cfg.agent.name);

    let addr = bind_addr(port);
    tracing::info!(%addr, agent = %cfg.agent.name, "roos serve starting");

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(handle_signals(shutdown_tx));

    // Background scheduler tick: log due tasks every second.
    let (stop_tx, mut stop_rx) = tokio::sync::watch::channel(());
    let sched_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    if let Ok(due) = scheduler.due_tasks() {
                        for t in &due {
                            tracing::info!(
                                task_id = %t.id,
                                agent   = %t.agent,
                                "scheduler: task due"
                            );
                        }
                    }
                }
                _ = stop_rx.changed() => break,
            }
        }
    });

    server
        .serve_with_shutdown(&addr, async {
            let _ = shutdown_rx.await;
        })
        .await
        .map_err(|e| anyhow::anyhow!("Server error: {e}"))?;

    let _ = stop_tx.send(());
    let _ = sched_handle.await;
    tracing::info!("roos serve stopped");
    Ok(())
}

async fn handle_signals(shutdown_tx: oneshot::Sender<()>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT listener");
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM listener");
        let mut sighup = signal(SignalKind::hangup()).expect("SIGHUP listener");
        loop {
            tokio::select! {
                _ = sigint.recv() => {
                    tracing::info!("received SIGINT, shutting down");
                    let _ = shutdown_tx.send(());
                    return;
                }
                _ = sigterm.recv() => {
                    tracing::info!("received SIGTERM, shutting down");
                    let _ = shutdown_tx.send(());
                    return;
                }
                _ = sighup.recv() => {
                    tracing::info!("received SIGHUP, reloading configuration");
                    // Config is re-read per-run; no restart needed.
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("ctrl_c listener error: {e}");
        }
        tracing::info!("received Ctrl-C, shutting down");
        let _ = shutdown_tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const SAMPLE_TOML: &str = r#"
[agent]
name = "my-agent"
description = "test"

[provider]
type = "openai"
model = "gpt-4o"
"#;

    fn write_config(tmp: &TempDir) -> String {
        let path = tmp.path().join("roos.toml");
        fs::write(&path, SAMPLE_TOML).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn bind_addr_formats_correctly() {
        assert_eq!(bind_addr(8080), "0.0.0.0:8080");
        assert_eq!(bind_addr(3000), "0.0.0.0:3000");
        assert_eq!(bind_addr(443), "0.0.0.0:443");
    }

    #[test]
    fn sched_db_path_beside_config() {
        let tmp = TempDir::new().unwrap();
        let config_path = write_config(&tmp);
        let db_path = sched_db_path(&config_path);
        assert_eq!(db_path.file_name().unwrap(), "roos-scheduler.db");
        assert_eq!(db_path.parent().unwrap(), tmp.path());
    }

    #[tokio::test]
    async fn missing_config_returns_error() {
        let err = run(8080, "/nonexistent/roos.toml", false)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Failed to load"));
    }
}
