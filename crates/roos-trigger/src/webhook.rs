use axum::{body::Body, extract::Request, http::StatusCode, middleware::Next, response::Response};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Extension holding the HMAC-SHA256 webhook secret.
#[derive(Clone)]
pub struct WebhookSecret(pub String);

/// Constant-time HMAC-SHA256 verification against a hex-encoded signature.
pub fn verify_hmac_sha256(secret: &[u8], body: &[u8], expected_hex: &str) -> bool {
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(body);
    let Ok(expected_bytes) = hex::decode(expected_hex) else {
        return false;
    };
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Verify a GitHub-style `sha256=<hex>` signature value.
pub fn verify_github_signature(secret: &[u8], body: &[u8], header_value: &str) -> bool {
    match header_value.strip_prefix("sha256=") {
        Some(hex) => verify_hmac_sha256(secret, body, hex),
        None => false,
    }
}

/// Axum middleware that verifies `X-Hub-Signature-256` (GitHub) or
/// `X-ROOS-Signature` (generic HMAC) when a [`WebhookSecret`] extension is
/// present on the route.  Passes through unchanged when no secret is configured.
pub async fn require_webhook_signature(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let secret = request
        .extensions()
        .get::<WebhookSecret>()
        .map(|s| s.0.clone());

    let Some(secret) = secret else {
        return Ok(next.run(request).await);
    };

    let github_sig = request
        .headers()
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let roos_sig = request
        .headers()
        .get("X-ROOS-Signature")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    if github_sig.is_none() && roos_sig.is_none() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Consume body to compute / compare HMAC.
    let (parts, body) = request.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let valid = match (github_sig.as_deref(), roos_sig.as_deref()) {
        (Some(sig), _) => verify_github_signature(secret.as_bytes(), &bytes, sig),
        (_, Some(sig)) => verify_hmac_sha256(secret.as_bytes(), &bytes, sig),
        _ => false,
    };

    if !valid {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Reconstruct the request with the already-consumed body.
    Ok(next
        .run(Request::from_parts(parts, Body::from(bytes)))
        .await)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compute_hex(secret: &[u8], body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret).unwrap();
        mac.update(body);
        hex::encode(mac.finalize().into_bytes())
    }

    #[test]
    fn valid_hmac_passes() {
        let hex = compute_hex(b"mysecret", b"payload");
        assert!(verify_hmac_sha256(b"mysecret", b"payload", &hex));
    }

    #[test]
    fn wrong_secret_fails() {
        let hex = compute_hex(b"mysecret", b"payload");
        assert!(!verify_hmac_sha256(b"wrongsecret", b"payload", &hex));
    }

    #[test]
    fn invalid_hex_fails() {
        assert!(!verify_hmac_sha256(
            b"mysecret",
            b"payload",
            "notvalidhex!!"
        ));
    }

    #[test]
    fn github_prefix_stripped_and_verified() {
        let hex = compute_hex(b"mysecret", b"payload");
        let header = format!("sha256={hex}");
        assert!(verify_github_signature(b"mysecret", b"payload", &header));
    }

    #[test]
    fn github_missing_prefix_fails() {
        let hex = compute_hex(b"mysecret", b"payload");
        // Missing "sha256=" prefix
        assert!(!verify_github_signature(b"mysecret", b"payload", &hex));
    }
}
