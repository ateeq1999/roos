use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
use uuid::Uuid;

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct TriggerRequest {
    pub agent: String,
    pub input: serde_json::Value,
}

#[derive(Serialize)]
pub struct TriggerResponse {
    pub run_id: String,
}

#[derive(Serialize, Clone)]
pub struct RunRecord {
    pub run_id: String,
    pub agent: String,
    pub status: String,
    pub output: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

#[derive(Serialize)]
struct AgentsResponse {
    agents: Vec<String>,
}

// ── App state ─────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct AppState {
    pub runs: Arc<RwLock<HashMap<Uuid, RunRecord>>>,
    pub agents: Arc<RwLock<Vec<String>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_agent(&self, name: impl Into<String>) {
        self.agents.write().unwrap().push(name.into());
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

async fn agents_handler(State(state): State<AppState>) -> Json<AgentsResponse> {
    let agents = state.agents.read().unwrap().clone();
    Json(AgentsResponse { agents })
}

async fn trigger_handler(
    State(state): State<AppState>,
    Json(req): Json<TriggerRequest>,
) -> (StatusCode, Json<TriggerResponse>) {
    let run_id = Uuid::new_v4();
    let record = RunRecord {
        run_id: run_id.to_string(),
        agent: req.agent,
        status: "pending".into(),
        output: None,
    };
    state.runs.write().unwrap().insert(run_id, record);
    (
        StatusCode::ACCEPTED,
        Json(TriggerResponse {
            run_id: run_id.to_string(),
        }),
    )
}

async fn runs_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RunRecord>, StatusCode> {
    let uuid = id.parse::<Uuid>().map_err(|_| StatusCode::BAD_REQUEST)?;
    let runs = state.runs.read().unwrap();
    runs.get(&uuid)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

// ── TriggerServer ─────────────────────────────────────────────────────────────

/// HTTP trigger server exposing `/health`, `/agents`, `/trigger`, and `/runs/{id}`.
pub struct TriggerServer {
    state: AppState,
}

impl TriggerServer {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
        }
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/health", get(health_handler))
            .route("/agents", get(agents_handler))
            .route("/trigger", post(trigger_handler))
            .route("/runs/:id", get(runs_handler))
            .with_state(self.state.clone())
    }

    pub async fn serve(self, addr: &str) -> Result<(), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router()).await
    }
}

impl Default for TriggerServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    async fn body_json(resp: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = TriggerServer::new().router();
        let req = Request::builder()
            .uri("/health")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn agents_returns_empty_list() {
        let app = TriggerServer::new().router();
        let req = Request::builder()
            .uri("/agents")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert!(json["agents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn trigger_creates_run() {
        let app = TriggerServer::new().router();
        let body = serde_json::json!({"agent": "my-agent", "input": {"task": "hello"}});
        let req = Request::builder()
            .method("POST")
            .uri("/trigger")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let json = body_json(resp).await;
        assert!(json["run_id"].as_str().is_some());
    }

    #[tokio::test]
    async fn runs_returns_not_found_for_unknown_id() {
        let app = TriggerServer::new().router();
        let id = Uuid::new_v4();
        let req = Request::builder()
            .uri(format!("/runs/{id}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn runs_returns_bad_request_for_invalid_id() {
        let app = TriggerServer::new().router();
        let req = Request::builder()
            .uri("/runs/not-a-uuid")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn trigger_then_runs_lookup() {
        let server = TriggerServer::new();
        let app = server.router();

        // POST /trigger
        let body = serde_json::json!({"agent": "alpha", "input": {}});
        let req = Request::builder()
            .method("POST")
            .uri("/trigger")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let run_json = body_json(resp).await;
        let run_id = run_json["run_id"].as_str().unwrap().to_owned();

        // GET /runs/{id}
        let req = Request::builder()
            .uri(format!("/runs/{run_id}"))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let status_json = body_json(resp).await;
        assert_eq!(status_json["agent"], "alpha");
        assert_eq!(status_json["status"], "pending");
    }
}
