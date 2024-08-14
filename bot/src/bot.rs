use crate::{process_message, FileQueueType};
use core::chat_config::PermissionsConfig;
use core::config::Config;
use log::{debug, error, info};
use reqwest::{Client, Url};
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::Message;
use tokio::sync::Mutex;

pub trait Bot {
    fn new(config: Arc<Config>, permissions: Arc<Mutex<PermissionsConfig>>, queue: FileQueueType) -> Self;
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
    fn new(config: Arc<Config>, permissions: Arc<Mutex<PermissionsConfig>>, queue: FileQueueType) -> Self {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(300))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|e| {
                error!("Failed to create client: {}", e);
                Client::new()
            });

        let mut bot = teloxide::Bot::with_client(config.bot_token().unwrap(), client);

        bot = bot.set_api_url(Url::parse(config.telegram_api_url().as_str()).unwrap());

        let bot_ref = Arc::new(bot);

        TeloxideBot {
            teloxide_bot: bot_ref,
            permissions,
            queue,
        }
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
