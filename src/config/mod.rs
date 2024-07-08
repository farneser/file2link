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
static INSTANCE: Lazy<RwLock<Option<Arc<Config>>>> = Lazy::new(|| RwLock::new(None));

impl Config {
    fn new() -> Self {
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
        println!("TELEGRAM_API_URL environment variable is not set");
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

#[cfg(test)]
mod test {
    use std::env;

    use super::*;

    fn set_env_variable(key: &str, value: &str) {
        env::set_var(key, value);
    }

    fn remove_env_variable(key: &str) {
        env::remove_var(key);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_bot_token_success() {
        set_env_variable("BOT_TOKEN", "test_token");

        let token = fetch_bot_token();

        assert_eq!(token, Ok("test_token".to_string()));

        remove_env_variable("BOT_TOKEN");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_bot_token_failure() {
        remove_env_variable("BOT_TOKEN");

        let token = fetch_bot_token();

        assert_eq!(token, Err("environment variable 'BOT_TOKEN' is not set".to_string()));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_server_port() {
        set_env_variable("SERVER_PORT", "9090");

        let port = fetch_server_port();

        assert_eq!(port, 9090);

        remove_env_variable("SERVER_PORT");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_server_port_default() {
        remove_env_variable("SERVER_PORT");

        let port = fetch_server_port();

        assert_eq!(port, 8080);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_domain() {
        set_env_variable("APP_DOMAIN", "http://example.com");

        let domain = fetch_domain();

        assert_eq!(domain, "http://example.com/");

        remove_env_variable("APP_DOMAIN");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_domain_default() {
        remove_env_variable("APP_DOMAIN");

        let domain = fetch_domain();
        let port = fetch_server_port();

        assert_eq!(domain, format!("http://localhost:{port}/"));
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_telegram_api() {
        set_env_variable("TELEGRAM_API_URL", "http://api.test.com");

        let api_url = fetch_telegram_api();

        assert_eq!(api_url, "http://api.test.com");

        remove_env_variable("TELEGRAM_API_URL");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_telegram_api_default() {
        remove_env_variable("TELEGRAM_API_URL");

        let api_url = fetch_telegram_api();

        assert_eq!(api_url, "https://api.telegram.org");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_pipe_path() {
        set_env_variable("F2L_PIPE_PATH", "/custom/path.pipe");

        let pipe_path = fetch_pipe_path();

        assert_eq!(pipe_path, "/custom/path.pipe");

        remove_env_variable("F2L_PIPE_PATH");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_pipe_path_default() {
        remove_env_variable("F2L_PIPE_PATH");

        let pipe_path = fetch_pipe_path();

        assert_eq!(pipe_path, "/tmp/file2link.pipe");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_enable_files_route_true() {
        set_env_variable("ENABLE_FILES_ROUTE", "true");

        let enable_files_route = fetch_enable_files_route();

        assert!(enable_files_route);

        remove_env_variable("ENABLE_FILES_ROUTE");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_enable_files_route_false() {
        set_env_variable("ENABLE_FILES_ROUTE", "false");

        let enable_files_route = fetch_enable_files_route();

        assert!(!enable_files_route);

        remove_env_variable("ENABLE_FILES_ROUTE");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_fetch_enable_files_route_default() {
        remove_env_variable("ENABLE_FILES_ROUTE");

        let enable_files_route = fetch_enable_files_route();

        assert!(!enable_files_route);
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_config_new() {
        set_env_variable("BOT_TOKEN", "test_token");
        set_env_variable("SERVER_PORT", "9090");
        set_env_variable("APP_DOMAIN", "http://example.com");
        set_env_variable("TELEGRAM_API_URL", "http://api.test.com");
        set_env_variable("F2L_PIPE_PATH", "/custom/path.pipe");
        set_env_variable("ENABLE_FILES_ROUTE", "true");

        let config = Config::new();

        assert_eq!(config.bot_token, Ok("test_token".to_string()));
        assert_eq!(config.server_port, 9090);
        assert_eq!(config.domain, "http://example.com/");
        assert_eq!(config.telegram_api_url, "http://api.test.com");
        assert_eq!(config.pipe_path, "/custom/path.pipe");
        assert!(config.enable_files_route);

        remove_env_variable("BOT_TOKEN");
        remove_env_variable("SERVER_PORT");
        remove_env_variable("APP_DOMAIN");
        remove_env_variable("TELEGRAM_API_URL");
        remove_env_variable("F2L_PIPE_PATH");
        remove_env_variable("ENABLE_FILES_ROUTE");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_config_instance() {
        set_env_variable("BOT_TOKEN", "test_token");
        set_env_variable("SERVER_PORT", "9090");
        set_env_variable("APP_DOMAIN", "http://example.com");
        set_env_variable("TELEGRAM_API_URL", "http://api.test.com");
        set_env_variable("F2L_PIPE_PATH", "/custom/path.pipe");
        set_env_variable("ENABLE_FILES_ROUTE", "true");

        let config = Config::instance().await;

        assert_eq!(config.bot_token.clone().expect(""), "test_token".to_string());
        assert_eq!(config.server_port, 9090);
        assert_eq!(config.domain, "http://example.com/");
        assert_eq!(config.telegram_api_url, "http://api.test.com");
        assert_eq!(config.pipe_path, "/custom/path.pipe");
        assert!(config.enable_files_route);

        remove_env_variable("BOT_TOKEN");
        remove_env_variable("SERVER_PORT");
        remove_env_variable("APP_DOMAIN");
        remove_env_variable("TELEGRAM_API_URL");
        remove_env_variable("F2L_PIPE_PATH");
        remove_env_variable("ENABLE_FILES_ROUTE");
    }
}
