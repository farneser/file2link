use axum::{Router, body::Body, response::Response, routing::get};
use http::StatusCode;
use std::path::Path;
use std::convert::Infallible;
use std::fs::File;
use std::io::{Read};

pub fn create_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/files/:id", get(move |path| files_id(path)))
}

async fn files_id(axum::extract::Path(path): axum::extract::Path<String>) -> Result<Response, Infallible> {
    return download_file(path).await;
}

async fn download_file(id: String) -> Result<Response<Body>, Infallible> {
    let file_path = format!("files/{}", id);
    let file_path = Path::new(&file_path);

    if !file_path.exists() {
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }

    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap());
        }
    };

    let mut buffer = Vec::new();

    if let Err(_) = file.read_to_end(&mut buffer) {
        return Ok(Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap());
    }

    let content_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Disposition", format!("attachment; filename=\"{}\"", file_path.file_name().unwrap_or_default().to_string_lossy()))
        .body(buffer.into())
        .unwrap())
}

async fn root() -> &'static str {
    "Server working"
}
