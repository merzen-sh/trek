use axum::extract::Request;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::rt::TokioExecutor;
use std::sync::LazyLock;

static CLIENT: LazyLock<Client<HttpConnector, Full<Bytes>>> =
    LazyLock::new(|| Client::builder(TokioExecutor::new()).build_http());

fn base_url() -> String {
    std::env::var("TREK_APP_PUBLIC_URL")
        .ok()
        .or_else(|| option_env!("TREK_APP_PUBLIC_URL").map(|url| url.to_string()))
        .unwrap_or_else(|| "http://localhost:5173".to_string())
        .trim()
        .to_string()
}

pub async fn handler(req: Request) -> Response {
    let base = base_url();
    let (parts, body) = req.into_parts();

    let path = parts.uri.path();
    let query = parts
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let url: hyper::Uri = match format!("{base}{path}{query}").parse() {
        Ok(uri) => uri,
        Err(e) => {
            tracing::warn!("invalid target uri: {e}");
            return (StatusCode::BAD_GATEWAY, "invalid upstream url format").into_response();
        }
    };

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            tracing::warn!("failed to read request body: {e}");
            return (StatusCode::BAD_REQUEST, "failed to read request body").into_response();
        }
    };

    let mut req_builder = Request::builder().method(parts.method).uri(url);
    for (key, value) in parts.headers.iter() {
        if !matches!(
            key.as_str(),
            "host"
                | "connection"
                | "transfer-encoding"
                | "upgrade"
                | "proxy-authorization"
                | "proxy-authenticate"
        ) {
            req_builder = req_builder.header(key, value);
        }
    }
    let req = req_builder.body(Full::new(body_bytes)).unwrap();

    match CLIENT.request(req).await {
        Ok(resp) => {
            let (resp_parts, resp_body) = resp.into_parts();
            let body = match resp_body.collect().await {
                Ok(collected) => collected.to_bytes(),
                Err(e) => {
                    tracing::warn!("failed to read upstream response body: {e}");
                    return (StatusCode::BAD_GATEWAY, "upstream read error").into_response();
                }
            };
            (resp_parts.status, resp_parts.headers, body).into_response()
        }
        Err(e) => {
            tracing::warn!("proxy error: {e}");
            (StatusCode::BAD_GATEWAY, format!("proxy error: {e}")).into_response()
        }
    }
}
