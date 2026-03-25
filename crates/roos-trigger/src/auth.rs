use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};

/// Axum middleware that requires a valid `Authorization: Bearer <token>` header.
///
/// The expected token is stored in [`BearerToken`] added to the router via
/// `axum::Extension` or passed through `AppState`. Here we receive it as an
/// Axum extension so callers can do:
///
/// ```ignore
/// router.layer(axum::middleware::from_fn_with_state(token, require_bearer))
/// ```
pub async fn require_bearer(request: Request, next: Next) -> Result<Response, StatusCode> {
    let token = request
        .extensions()
        .get::<BearerToken>()
        .map(|t| t.0.as_str());

    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match (token, provided) {
        (Some(expected), Some(actual)) if expected == actual => Ok(next.run(request).await),
        (None, _) => Ok(next.run(request).await), // no token configured → open
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

/// Newtype wrapper around the expected bearer token string.
///
/// Insert this as an Axum `Extension` before the auth middleware layer so
/// `require_bearer` can retrieve it from the request extensions.
#[derive(Clone)]
pub struct BearerToken(pub String);

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, middleware, routing::get, Extension, Router};
    use tower::ServiceExt;

    async fn ok_handler() -> StatusCode {
        StatusCode::OK
    }

    fn make_app(token: Option<&str>) -> Router {
        let router = Router::new()
            .route("/protected", get(ok_handler))
            .layer(middleware::from_fn(require_bearer));

        if let Some(t) = token {
            router.layer(Extension(BearerToken(t.to_owned())))
        } else {
            router
        }
    }

    #[tokio::test]
    async fn no_configured_token_is_open() {
        let app = make_app(None);
        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn valid_token_passes() {
        let app = make_app(Some("secret"));
        let req = Request::builder()
            .uri("/protected")
            .header("Authorization", "Bearer secret")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn wrong_token_is_unauthorized() {
        let app = make_app(Some("secret"));
        let req = Request::builder()
            .uri("/protected")
            .header("Authorization", "Bearer wrong")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_header_is_unauthorized() {
        let app = make_app(Some("secret"));
        let req = Request::builder()
            .uri("/protected")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn malformed_scheme_is_unauthorized() {
        let app = make_app(Some("secret"));
        let req = Request::builder()
            .uri("/protected")
            .header("Authorization", "Basic secret")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }
}
