use std::{fs, path::PathBuf};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
};
use tower::ServiceExt;

async fn get(path: &str) -> Response {
    flip::api::router()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn dist() -> PathBuf {
    // rust-embed reads these built files from disk in debug builds.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("frontend/dist")
}

#[tokio::test]
async fn embedded_index_serves_html() {
    let expected = fs::read(dist().join("index.html")).expect("build frontend/dist before testing");
    let response = get("/").await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "text/html");
    let body = to_bytes(response.into_body(), expected.len())
        .await
        .unwrap();
    assert_eq!(body.as_ref(), expected);
}

#[tokio::test]
async fn built_javascript_asset_serves_with_its_mime_type() {
    let asset = fs::read_dir(dist().join("assets"))
        .expect("build frontend/dist before testing")
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "js"))
        .expect("frontend build contains a JavaScript asset");
    let expected = fs::read(&asset).unwrap();
    let path = format!("/assets/{}", asset.file_name().unwrap().to_str().unwrap());
    let response = get(&path).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "text/javascript");
    let body = to_bytes(response.into_body(), expected.len())
        .await
        .unwrap();
    assert_eq!(body.as_ref(), expected);
}

#[tokio::test]
async fn missing_assets_return_not_found() {
    for path in ["/assets/does-not-exist.js", "/assets/unknown.bin"] {
        assert_eq!(get(path).await.status(), StatusCode::NOT_FOUND, "{path}");
    }
}
