use std::{convert::Infallible, fs::{self, File}, io::Read, path::PathBuf};

use axum::{
    body::Body,
    extract,
    response::{Html, Response},
    routing::{get, Router},
};
use axum::response::IntoResponse;
use http::{header::CONTENT_TYPE, StatusCode};
use log::{debug, error, info, warn};
use mime_guess::from_path;

use crate::utils;

pub fn create_app() -> Router {
    let enable_files_route = utils::fetch_enable_files_route();

    let mut router = Router::new()
        .route("/", get(root))
        .route("/files/:id", get(files_id));

    if enable_files_route {
        router = router.route("/files", get(files_list));
    }

    router.fallback(not_found_handler)
}

/// ignores folders and shows only files
async fn files_list() -> Result<Response<Body>, Infallible> {
    info!("Files list accessed");

    let folder_path = PathBuf::from("files");

    debug!("Listing files in folder path: {:?}", folder_path);

    if !folder_path.is_dir() {
        warn!("Path is not a directory: {:?}", folder_path);

        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::empty())
            .unwrap());
    }

    let entries = match fs::read_dir(&folder_path) {
        Ok(entries) => entries,
        Err(e) => {
            error!("Failed to read directory: {:?}. Error: {}", folder_path, e);

            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap());
        }
    };

    let mut html = String::from("<h1>Files in directory</h1><ul>");

    for entry in entries {
        match entry {
            Ok(entry) => {
                let path = entry.path();

                if path.is_file() {
                    if let Some(file_name) = path.file_name() {
                        let file_name = file_name.to_string_lossy();

                        html.push_str(&format!("<li><a href=\"/files/{}\">{}</a></li>", file_name, file_name));
                    }
                }
            }
            Err(e) => {
                error!("Failed to read directory entry: {:?}. Error: {}", folder_path, e);

                continue;
            }
        }
    }

    html.push_str("</ul>");

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap())
}

async fn files_id(extract::Path(id): extract::Path<String>) -> Result<Response<Body>, Infallible> {
    let file_path = format!("files/{}", id);
    let file_path = PathBuf::from(&file_path);

    debug!("Requested file path: {:?}", file_path);

    if !file_path.exists() {
        warn!("File not found: {:?}", file_path);

        let body = not_found_handler().await;

        return Ok((
            StatusCode::NOT_FOUND,
            [(CONTENT_TYPE, "text/html")],
            body,
        ).into_response());
    }

    let mut file = match File::open(&file_path) {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open file: {:?}. Error: {}", file_path, e);

            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap());
        }
    };

    let mut buffer = Vec::new();

    if let Err(e) = file.read_to_end(&mut buffer) {
        error!("Failed to read file: {:?}. Error: {}", file_path, e);

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

    info!("Serving file: {:?} with content type: {}", file_path, content_type);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("Content-Disposition", content_disposition)
        .body(buffer.into())
        .unwrap())
}

async fn root() -> Html<&'static str> {
    info!("Root path accessed");

    Html("\
    <h1>Server working</h1>\
    <div><a href=\"https://github.com/farneser/file2link\">GitHub</a></div>\
    ")
}

async fn not_found_handler() -> Html<&'static str> {
    Html("\
    <h1>404 Not Found</h1>\
    <p>The page you are looking for does not exist.</p>\
    <a href=\"/\">Go back to the homepage</a>\
    ")
}