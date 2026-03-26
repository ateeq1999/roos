use axum::{
    extract::{Path, State},
    http::StatusCode,
    middleware,
    routing::{get, post},
    Extension, Json, Router,
};

use crate::auth::{require_bearer, BearerToken};
use crate::webhook::{require_webhook_signature, WebhookSecret};
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
///
/// When a `bearer_token` is configured (via [`TriggerServer::with_token`]),
/// all routes except `/health` require `Authorization: Bearer <token>`.
pub struct TriggerServer {
    state: AppState,
    bearer_token: Option<String>,
    webhook_secret: Option<String>,
}

impl TriggerServer {
    pub fn new() -> Self {
        Self {
            state: AppState::new(),
            bearer_token: None,
            webhook_secret: None,
        }
    }

    /// Require Bearer token authentication on protected routes.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.bearer_token = Some(token.into());
        self
    }

    /// Enable HMAC-SHA256 webhook signature verification on the `/trigger` route.
    /// Accepts `X-Hub-Signature-256` (GitHub) or `X-ROOS-Signature` (generic).
    pub fn with_webhook_secret(mut self, secret: impl Into<String>) -> Self {
        self.webhook_secret = Some(secret.into());
        self
    }

    pub fn state(&self) -> &AppState {
        &self.state
    }

    pub fn router(&self) -> Router {
        // `/trigger` optionally requires HMAC-SHA256 webhook signature.
        let trigger = Router::new().route("/trigger", post(trigger_handler));
        let trigger = if let Some(ref secret) = self.webhook_secret {
            trigger
                .layer(middleware::from_fn(require_webhook_signature))
                .layer(Extension(WebhookSecret(secret.clone())))
        } else {
            trigger
        };

        // /health is always open; remaining routes are protected when a token is set.
        let protected = Router::new()
            .route("/agents", get(agents_handler))
            .merge(trigger)
            .route("/runs/:id", get(runs_handler))
            .layer(middleware::from_fn(require_bearer));

        let protected = if let Some(ref t) = self.bearer_token {
            protected.layer(Extension(BearerToken(t.clone())))
        } else {
            protected
        };

        Router::new()
            .route("/health", get(health_handler))
            .merge(protected)
            .with_state(self.state.clone())
    }

    pub async fn serve(self, addr: &str) -> Result<(), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router()).await
    }

    /// Like [`serve`] but resolves `shutdown` for graceful shutdown.
    pub async fn serve_with_shutdown<F>(self, addr: &str, shutdown: F) -> Result<(), std::io::Error>
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown)
            .await
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

    // ── webhook signature tests ───────────────────────────────────────────────

    fn github_sig(secret: &[u8], body: &[u8]) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        let mut mac = Hmac::<Sha256>::new_from_slice(secret).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[tokio::test]
    async fn webhook_valid_github_sig_passes() {
        let app = TriggerServer::new().with_webhook_secret("s3cr3t").router();
        let body = serde_json::json!({"agent": "bot", "input": {}}).to_string();
        let sig = github_sig(b"s3cr3t", body.as_bytes());
        let req = Request::builder()
            .method("POST")
            .uri("/trigger")
            .header("content-type", "application/json")
            .header("X-Hub-Signature-256", sig)
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
    }

    #[tokio::test]
    async fn webhook_missing_sig_is_unauthorized() {
        let app = TriggerServer::new().with_webhook_secret("s3cr3t").router();
        let body = serde_json::json!({"agent": "bot", "input": {}}).to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/trigger")
            .header("content-type", "application/json")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn webhook_invalid_sig_is_unauthorized() {
        let app = TriggerServer::new().with_webhook_secret("s3cr3t").router();
        let body = serde_json::json!({"agent": "bot", "input": {}}).to_string();
        let req = Request::builder()
            .method("POST")
            .uri("/trigger")
            .header("content-type", "application/json")
            .header("X-Hub-Signature-256", "sha256=deadbeef")
            .body(Body::from(body))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
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
