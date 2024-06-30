use axum::{Router, response::{Response, Html}, routing::get, extract};
use http::{StatusCode, header::CONTENT_TYPE};
use std::{path::PathBuf, convert::Infallible, fs::File, io::{Read}};
use axum::body::Body;
use mime_guess::from_path;

pub fn create_app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/files/:id", get(files_id))
}

async fn files_id(extract::Path(id): extract::Path<String>) -> Result<Response<Body>, Infallible> {
    let file_path = format!("files/{}", id);
    let file_path = PathBuf::from(&file_path);

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

    let content_type = from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
    let content_disposition = format!("attachment; filename=\"{}\"", file_name);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("Content-Disposition", content_disposition)
        .body(buffer.into())
        .unwrap())
}

async fn root() -> Html<&'static str> {
    Html("\
    <h1>Server working</h1>\
    <div><a href=\"https://github.com/farneser/file2link\">GitHub</a></div>\
    ")
}
