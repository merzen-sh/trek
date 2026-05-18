use axum::Router;
use axum::routing::get;

#[cfg(feature = "swagger")]
mod swagger {
    use axum::Router;
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    #[derive(OpenApi)]
    #[openapi(info(title = "trek API", version = "0.1.0"))]
    struct ApiDoc;

    pub fn router() -> Router {
        Router::new()
            .merge(SwaggerUi::new("/swagger-ui").url("/api/openapi.json", ApiDoc::openapi()))
    }
}

mod proxy {
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
        std::env::var("TREK_APP_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:5173".to_string())
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
}

pub fn create() -> Router {
    let router = Router::new().route("/api/health", get(|| async { "OK" }));

    #[cfg(feature = "swagger")]
    let router = router.merge(swagger::router());

    // fallback to SPA at / - SPA handles all client-side routing
    let router = router.fallback(proxy::handler);

    router
}
