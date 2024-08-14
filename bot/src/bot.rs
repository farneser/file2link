use crate::{process_message, FileQueueType};
use log::{debug, error, info};
use reqwest::{Client, Url};
use shared::chat_config::PermissionsConfig;
use shared::config::Config;
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::Message;
use tokio::sync::Mutex;

pub trait Bot {
    fn new(config: Arc<Config>, permissions: Arc<Mutex<PermissionsConfig>>, queue: FileQueueType) -> Result<Self, String> where Self: Sized;
    fn run(&self, tx: tokio::sync::mpsc::Sender<()>) -> impl std::future::Future<Output=()> + Send;
}

#[derive(Debug, Clone)]
pub struct TeloxideBot {
    permissions: Arc<Mutex<PermissionsConfig>>,
    queue: FileQueueType,
    teloxide_bot: Arc<teloxide::Bot>,
}

impl TeloxideBot {
    pub fn get_teloxide_bot(&self) -> Arc<teloxide::Bot> {
        self.teloxide_bot.clone()
    }
}

impl Bot for TeloxideBot {
    fn new(config: Arc<Config>, permissions: Arc<Mutex<PermissionsConfig>>, queue: FileQueueType) -> Result<Self, String> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(300))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|e| {
                error!("Failed to create client: {}", e);
                Client::new()
            });

        let token = match config.bot_token() {
            Ok(t) => { t }
            Err(_) => {
                error!("Failed to get bot token");

                return Err("Failed to get bot token".to_owned());
            }
        };

        let mut bot = teloxide::Bot::with_client(token, client);

        bot = bot.set_api_url(Url::parse(config.telegram_api_url().as_str()).unwrap());

        let bot_ref = Arc::new(bot);

        Ok(TeloxideBot {
            teloxide_bot: bot_ref,
            permissions,
            queue,
        })
    }

    async fn run(&self, tx: tokio::sync::mpsc::Sender<()>) {
        let file_queue = Arc::clone(&self.queue);
        let permissions = Arc::clone(&self.permissions);
        let bot = self.teloxide_bot.clone();

        teloxide::repl(bot.clone(), move |msg: Message| {
            debug!("Received message: {:?}", msg);

            let bot = Arc::clone(&bot);
            let bot_clone = Arc::clone(&bot);
            let permissions = Arc::clone(&permissions);
            let file_queue = Arc::clone(&file_queue);
            let tx = tx.clone();

            async move {
                let permissions = permissions.lock().await;

                let from = match msg.from() {
                    Some(from) => from,
                    None => {
                        info!("Message does not have a sender");
                        return Ok(());
                    }
                };

                if !permissions.user_has_access(msg.chat.id.to_string(), &from.id.to_string()) {
                    info!(
                        "User {} does not have access to chat {}",
                        msg.from().unwrap().id,
                        msg.clone().chat.id
                    );

                    return Ok(());
                }

                info!(
                    "User {} has access to chat {}",
                    msg.from().unwrap().id,
                    msg.clone().chat.id
                );

                if let Err(e) = process_message(bot_clone.clone(), msg.clone(), file_queue, tx).await {
                    error!("Failed to process message: {}", e);
                }

                Ok(())
            }
        }).await;
    }
}

#[cfg(test)]
mod tests {
    use crate::bot::{Bot, TeloxideBot};
    use shared::chat_config::PermissionsConfig;
    use shared::config::Config;
    use std::env;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[tokio::test]
    #[serial_test::serial]
    async fn test_teloxide_bot_new() {
        env::set_var("BOT_TOKEN", "test_token");

        let config = Arc::new(Config::new());
        let permissions = Arc::new(Mutex::new(PermissionsConfig::init_allow_all()));
        let queue = Arc::new(Mutex::new(Vec::new()));

        let bot = match TeloxideBot::new(config, permissions, queue) {
            Ok(b) => { b }
            Err(_) => {
                panic!("Failed to create bot");
            }
        };

        assert_eq!(bot.get_teloxide_bot().token(), "test_token");
        assert_eq!(bot.get_teloxide_bot().api_url().as_str(), "https://api.telegram.org/");

        env::remove_var("BOT_TOKEN")
    }
}