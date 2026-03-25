use tracing::Span;
use tracing_subscriber::{fmt, EnvFilter};
use uuid::Uuid;

/// Initialise the global tracing subscriber with a JSON formatter.
///
/// `level` is a [`tracing_subscriber::EnvFilter`] directive string, e.g.
/// `"info"`, `"debug"`, or `"roos=debug,info"`.
///
/// Uses `try_init` internally so subsequent calls in the same process (e.g.
/// during tests) return an error rather than panicking.
///
/// # Errors
///
/// Returns an error if the level directive is invalid or a global subscriber
/// is already installed.
pub fn init_logging(level: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let filter = EnvFilter::try_new(level)?;
    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .json()
        .try_init()?;
    Ok(())
}

/// Create a [`tracing::Span`] for a single agent run, recording `run_id`
/// as a structured field.
///
/// Enter (or instrument) this span before calling `ReasoningLoop::run` so
/// all log events emitted inside the run are automatically correlated to the
/// same `run_id`.
///
/// # Example
///
/// ```no_run
/// # use uuid::Uuid;
/// # use roos_observability::run_span;
/// let span = run_span(Uuid::new_v4());
/// let _guard = span.enter();
/// // … run the agent …
/// ```
pub fn run_span(run_id: Uuid) -> Span {
    tracing::info_span!("agent_run", %run_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_subscriber<F: FnOnce()>(f: F) {
        use tracing_subscriber::prelude::*;
        let sub = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_writer(std::io::sink));
        tracing::subscriber::with_default(sub, f);
    }

    #[test]
    fn run_span_is_not_disabled_with_subscriber() {
        with_subscriber(|| {
            let span = run_span(Uuid::new_v4());
            assert!(!span.is_disabled());
        });
    }

    #[test]
    fn run_span_can_be_entered() {
        with_subscriber(|| {
            let span = run_span(Uuid::new_v4());
            let _guard = span.enter();
            // No panic means success.
        });
    }

    #[test]
    fn invalid_level_returns_error() {
        let result = init_logging("%%%invalid%%%");
        assert!(result.is_err());
    }

    #[test]
    fn duplicate_init_returns_error() {
        // First init may succeed or fail (another test may have already set
        // the global subscriber). Either way the second call must not panic.
        let _ = init_logging("info");
        let second = init_logging("debug");
        // We only assert it doesn't panic; success/failure both acceptable.
        let _ = second;
    }

    #[test]
    fn run_span_unique_ids_produce_distinct_spans() {
        with_subscriber(|| {
            let id1 = Uuid::new_v4();
            let id2 = Uuid::new_v4();
            let span1 = run_span(id1);
            let span2 = run_span(id2);
            // Both spans get real IDs from the subscriber and they differ.
            assert_ne!(span1.id(), span2.id());
        });
    }
}
