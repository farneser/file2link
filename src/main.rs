use std::convert::Infallible;
use std::{env, process};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use axum::body::Body;
use axum::response::Response;
use axum::Router;
use axum::routing::get;
use dotenv::dotenv;
use http::StatusCode;
use reqwest::Client;
use teloxide::{Bot, prelude::*};
use tokio::{fs};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    load_env();

    let token = match fetch_bot_token() {
        Ok(token) => token,
        Err(e) => {
            eprintln!("Failed to fetch bot token: {}", e);
            process::exit(1);
        }
    };

    let server_port = fetch_server_port();

    let bot = Bot::new(token);

    let bot_task = tokio::spawn(async move {
        teloxide::repl(bot, |bot: Bot, msg: Message| async move {
            process_message(bot, msg).await.expect("TODO: panic message");
            Ok(())
        })
            .await;
    });

    let server_task = tokio::spawn(async move {
        let app = Router::new()
            .route("/", get(root))
            .route("/files/:id", get(move |path| files_id(path)));

        let addr: String = format!("0.0.0.0:{server_port}").parse().unwrap();

        let listener = TcpListener::bind(addr).await.unwrap();

        axum::serve(listener, app).await.unwrap();
    });

    tokio::select! {
        _ = bot_task => {},
        _ = server_task => {},
    }
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
    "Hello, World!"
}

fn load_env() {
    let dotenv_path = ".env";

    if Path::new(dotenv_path).exists() {
        dotenv().expect("Failed to read '.env' file");
    }
}

fn fetch_env_variable(var: &str) -> Option<String> {
    env::var(var).ok()
}

fn fetch_bot_token() -> Result<String, String> {
    let val = fetch_env_variable("BOT_TOKEN");

    match val {
        None => Err("environment variable 'BOT_TOKEN' is not set".to_owned()),
        Some(_) => Ok(val.unwrap())
    }
}

fn fetch_server_port() -> i16 {
    fetch_env_variable("SERVER_PORT")
        .and_then(|val| val.parse().ok())
        .unwrap_or(8080)
}

fn fetch_domain() -> String {
    let default_port = fetch_server_port();

    let default_url = format!("http://localhost:{default_port}");

    let domain = fetch_env_variable("APP_DOMAIN").unwrap_or_else(|| default_url);

    if domain.ends_with('/') {
        domain
    } else {
        format!("{domain}/")
    }
}


async fn process_message(bot: Bot, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(document) = msg.document() {
        handle_file(&bot, &msg, document.file.id.clone(), document.file_name.as_ref()).await?;
    } else if let Some(photo) = msg.photo().and_then(|p| p.last()) {
        handle_file(&bot, &msg, photo.file.id.clone(), None).await?;
    } else if let Some(video) = msg.video() {
        handle_file(&bot, &msg, video.file.id.clone(), video.file_name.as_ref()).await?;
    } else if let Some(animation) = msg.animation() {
        handle_file(&bot, &msg, animation.file.id.clone(), animation.file_name.as_ref()).await?;
    }

    Ok(())
}

async fn handle_file(
    bot: &Bot,
    msg: &Message,
    file_id: String,
    file_name: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    bot.send_message(msg.chat.id, "Starting file downloading").await?;

    let file_info = bot.get_file(file_id).await.unwrap();

    let file_url = format!("https://api.telegram.org/file/bot{}/{}", bot.token(), file_info.path);

    let bytes = fetch_file_bytes(&file_url).await?;
    let uuid = Uuid::new_v4();

    let final_file_name = match file_name {
        Some(name) => format!("files/{}_{}", uuid, name),
        None => format!("files/{}_{}", uuid, get_file_name_from_path(&file_info.path).unwrap()),
    };

    create_directory("files").await.expect("Failed to create directory 'files'");
    save_file(&final_file_name, &bytes)?;

    let final_size = get_file_size(&final_file_name).await?;

    println!("File saved: {:?}", final_file_name);

    bot.send_message(
        msg.chat.id,
        format!(
            "Downloaded. Size: {} bites\n{}{}",
            final_size.to_string(),
            fetch_domain(),
            final_file_name
        ),
    ).await?;

    Ok(())
}

fn get_file_name_from_path(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

async fn get_file_size(path: &str) -> io::Result<u64> {
    let metadata = fs::metadata(path).await?;

    Ok(metadata.len())
}

async fn fetch_file_bytes(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let client = Client::new();
    let response = client.get(url).send().await?;
    let bytes = response.bytes().await?;

    Ok(bytes.to_vec())
}

async fn create_directory(dir_name: &str) -> io::Result<()> {
    fs::create_dir_all(dir_name).await?;

    Ok(())
}

fn save_file(file_path: &str, bytes: &[u8]) -> io::Result<()> {
    let mut file = File::create(file_path)?;

    file.write_all(bytes)?;

    Ok(())
}
