// roos-observability — tracing, OTel export, Prometheus metrics.

pub mod logging;
pub use logging::{init_logging, run_span};
