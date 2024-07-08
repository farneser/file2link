use std::env;
use std::path::Path;
use std::sync::Arc;

use dotenv::dotenv;
use log::{info, warn};
use once_cell::sync::Lazy;
use tokio::sync::RwLock;

pub struct Config {
    bot_token: Result<String, String>,
    server_port: i16,
    domain: String,
    telegram_api_url: String,
    pipe_path: String,
    enable_files_route: bool,
}

impl Config {
    fn new() -> Self {
        load_env();

        let bot_token = fetch_bot_token();

        let server_port = fetch_server_port();
        let domain = fetch_domain();
        let telegram_api_url = fetch_telegram_api();
        let pipe_path = fetch_pipe_path();
        let enable_files_route = fetch_enable_files_route();

        Self {
            bot_token,
            server_port,
            domain,
            telegram_api_url,
            pipe_path,
            enable_files_route,
        }
    }

    pub async fn instance() -> Arc<Config> {
        static INSTANCE: Lazy<RwLock<Option<Arc<Config>>>> = Lazy::new(|| RwLock::new(None));

        let mut instance = INSTANCE.write().await;

        if instance.is_none() {
            *instance = Some(Arc::new(Config::new()));
        }

        instance.clone().unwrap()
    }

    pub fn bot_token(&self) -> Result<String, String> {
        self.bot_token.to_owned()
    }

    pub fn server_port(&self) -> i16 {
        self.server_port
    }

    pub fn domain(&self) -> String {
        self.domain.to_owned()
    }

    pub fn telegram_api_url(&self) -> String {
        self.telegram_api_url.to_owned()
    }

    pub fn pipe_path(&self) -> String {
        self.pipe_path.to_owned()
    }

    pub fn enable_files_route(&self) -> bool {
        self.enable_files_route
    }
}

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

fn fetch_telegram_api() -> String {
    fetch_env_variable("TELEGRAM_API_URL").unwrap_or_else(|| {
        println!("API_URL environment variable is not set");
        "https://api.telegram.org".to_owned()
    })
}

fn fetch_pipe_path() -> String {
    fetch_env_variable("F2L_PIPE_PATH").unwrap_or_else(|| {
        info!("F2L_PIPE_PATH environment variable is not set");
        "/tmp/file2link.pipe".to_owned()
    })
}

fn fetch_enable_files_route() -> bool {
    fetch_env_variable("ENABLE_FILES_ROUTE")
        .unwrap_or_else(|| {
            warn!("ENABLE_FILES_ROUTE environment variable is not set. Defaulting to false.");
            "false".to_owned()
        })
        .parse()
        .unwrap_or(false)
}

