use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::RustEmbed;

// Debug builds read frontend/dist from disk. Release builds embed it at compile time.
#[derive(RustEmbed)]
#[folder = "frontend/dist"]
struct Frontend;

pub(crate) fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/*path", get(asset))
}

async fn index() -> Response {
    serve("index.html")
}

async fn asset(Path(path): Path<String>) -> Response {
    serve(&format!("assets/{path}"))
}

fn serve(path: &str) -> Response {
    let Some(content_type) = content_type(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match Frontend::get(path) {
        Some(file) => (
            [(header::CONTENT_TYPE, content_type)],
            file.data.into_owned(),
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn content_type(path: &str) -> Option<&'static str> {
    match path.rsplit_once('.')?.1 {
        "html" => Some("text/html"),
        "js" => Some("text/javascript"),
        "css" => Some("text/css"),
        "svg" => Some("image/svg+xml"),
        "ico" => Some("image/x-icon"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "woff2" => Some("font/woff2"),
        "json" => Some("application/json"),
        "txt" => Some("text/plain"),
        _ => None,
    }
}
