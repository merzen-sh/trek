use axum::Router;
use axum::routing::get;

#[cfg(feature = "swagger")]
mod swagger {
    use axum::Router;
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    #[derive(OpenApi)]
    #[openapi(
        info(
            title = "trek API",
            version = "0.1.0",
            description = "Trek API Documentation"
        ),
        paths(crate::server::health::handler)
    )]
    struct ApiDoc;

    pub fn router() -> Router {
        Router::new()
            .merge(SwaggerUi::new("/swagger-ui").url("/api/openapi.json", ApiDoc::openapi()))
    }
}

pub fn create() -> Router {
    let router = Router::new().route("/api/health", get(crate::server::health::handler));

    #[cfg(feature = "swagger")]
    let router = router.merge(swagger::router());

    let router = router.fallback(crate::server::proxy::handler);

    router
}
