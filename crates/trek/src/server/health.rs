use axum::response::IntoResponse;

#[cfg_attr(
    feature = "swagger",
    utoipa::path(
        get,
        path = "/api/health",
        tag = "General",
        responses(
            (status = 200, description = "Health check OK", body = String)
        )
    )
)]
pub async fn handler() -> impl IntoResponse {
    "OK"
}
