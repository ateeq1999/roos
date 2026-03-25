// roos-trigger — HTTP trigger server and webhook handlers.

pub mod server;
pub use server::{AppState, RunRecord, TriggerRequest, TriggerResponse, TriggerServer};
