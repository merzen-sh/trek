use axum::Router;

#[cfg(debug_assertions)]
mod swagger {
    use axum::Router;
    use utoipa::OpenApi;
    use utoipa_swagger_ui::SwaggerUi;

    #[derive(OpenApi)]
    #[openapi(info(title = "trek API", version = "0.1.0"), paths(super::handle_root))]
    struct ApiDoc;

    pub fn apply(router: Router) -> Router {
        router.merge(SwaggerUi::new("/swagger-ui").url("/api-doc/openapi.json", ApiDoc::openapi()))
    }
}

#[utoipa::path(
    get,
    path = "/",
    responses(
        (status = 200, description = "hello world response", body = str),
    ),
)]
pub async fn handle_root() -> &'static str {
    "hello world!"
}

pub fn create() -> Router {
    let router = Router::new().route("/", axum::routing::get(handle_root));

    #[cfg(debug_assertions)]
    let router = swagger::apply(router);

    router
}
