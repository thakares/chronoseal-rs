use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

pub async fn log_request(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let response = next.run(req).await;
    tracing::info!("{} {} -> {}", method, uri, response.status());
    response
}

/// Injects defensive HTTP response headers on every response.
///
/// These headers mitigate several classes of attacks:
/// - `X-Content-Type-Options: nosniff` — prevents MIME-type sniffing.
/// - `X-Frame-Options: DENY` — blocks clickjacking via framing.
/// - `Referrer-Policy: no-referrer` — suppresses referrer leakage.
/// - `X-XSS-Protection: 0` — disables legacy XSS auditors (can introduce bugs).
/// - `Permissions-Policy` — restricts powerful browser features.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("referrer-policy", "no-referrer".parse().unwrap());
    headers.insert("x-xss-protection", "0".parse().unwrap());
    headers.insert(
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()".parse().unwrap(),
    );
    response
}
