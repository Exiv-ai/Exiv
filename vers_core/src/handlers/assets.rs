use rust_embed::RustEmbed;
use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};

#[derive(RustEmbed)]
#[folder = "../vers_dashboard/dist/"]
struct Asset;

pub async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // 1. 指定されたパスを検索
    // 2. なければ index.html を返す（SPA対応）
    if let Some(file) = Asset::get(path) {
        let mime_type = mime_guess::from_path(path).first_or_octet_stream();
        Response::builder()
            .header(header::CONTENT_TYPE, mime_type.as_ref())
            .body(Body::from(file.data))
            .unwrap()
    } else {
        // Fallback to index.html for SPA routing
        match Asset::get("index.html") {
            Some(index) => {
                let mime_type = mime_guess::from_path("index.html").first_or_octet_stream();
                Response::builder()
                    .header(header::CONTENT_TYPE, mime_type.as_ref())
                    .body(Body::from(index.data))
                    .unwrap()
            },
            None => {
                StatusCode::NOT_FOUND.into_response()
            }
        }
    }
}
