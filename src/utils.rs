use std::env;
use std::io;
use std::path::Path;

use dotenv::dotenv;
use log::{info, warn};
use tokio::fs;

pub fn load_env() {
    fn load_log_level() {
        let default_log_level = "info";
        let log_level = env::var("RUST_LOG").unwrap_or_else(|_| default_log_level.to_string());

        env::set_var("RUST_LOG", log_level);
    }

    let dotenv_path = ".env";

    if Path::new(dotenv_path).exists() {
        dotenv().expect("Failed to read '.env' file");

        info!("Successfully loaded .env file");
    } else {
        warn!("Failed to find .env file. Using system environment variables instead.");
    }

    load_log_level();
}

pub fn fetch_env_variable(var: &str) -> Option<String> {
    env::var(var).ok()
}

pub fn fetch_bot_token() -> Result<String, String> {
    let val = fetch_env_variable("BOT_TOKEN");

    match val {
        None => Err("environment variable 'BOT_TOKEN' is not set".to_owned()),
        Some(_) => Ok(val.unwrap())
    }
}

pub fn fetch_server_port() -> i16 {
    fetch_env_variable("SERVER_PORT")
        .and_then(|val| val.parse().ok())
        .unwrap_or(8080)
}

pub fn fetch_domain() -> String {
    let default_port = fetch_server_port();

    let default_url = format!("http://localhost:{default_port}");

    let domain = fetch_env_variable("APP_DOMAIN").unwrap_or_else(|| default_url);

    if domain.ends_with('/') {
        domain
    } else {
        format!("{domain}/")
    }
}

pub fn fetch_telegram_api() -> String {
    fetch_env_variable("TELEGRAM_API_URL").unwrap_or_else(|| {
        println!("API_URL environment variable is not set");
        "https://api.telegram.org".to_owned()
    })
}

pub fn get_file_name_from_path(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

pub async fn get_file_size(path: &str) -> io::Result<u64> {
    let metadata = fs::metadata(path).await?;

    Ok(metadata.len())
}

pub async fn create_directory(dir_name: &str) -> io::Result<()> {
    fs::create_dir_all(dir_name).await?;

    Ok(())
}

pub fn get_folder_and_file_name(path: &str) -> Option<String> {
    let path = Path::new(path);

    let parent_dir = path.parent()?.file_name()?.to_string_lossy().into_owned();

    let file_name = path.file_name()?.to_string_lossy().into_owned();

    Some(format!("{}/{}", parent_dir, file_name))
}
