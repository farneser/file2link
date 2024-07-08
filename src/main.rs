use std::error::Error;
use std::sync::Arc;

use log::{debug, error, info};
use teloxide::Bot;
use teloxide::prelude::Message;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};

use crate::bot::FileQueueType;
use crate::cli::cli::send_command;
use crate::cli::handle_cli;
use crate::config::Config;

mod bot;
mod server;
mod utils;
mod chat_config;
mod cli;
mod config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    config::load_env();

    pretty_env_logger::init();

    info!("Starting up...");

    let server_port = Config::instance().await.server_port();
    info!("Server port: {}", server_port);

    let raw_permissions = chat_config::load_config()
        .await.expect("Failed to load config");

    let permissions = Arc::new(Mutex::new(raw_permissions));

    let bot = match bot::get_bot().await {
        Ok(bot) => bot,
        Err(e) => {
            error!("Failed to create bot: {}", e);

            return Err("Failed to create bot".into());
        }
    };

    let file_queue: FileQueueType = Arc::new(Mutex::new(Vec::new()));

    let (tx, rx) = mpsc::channel(100);

    let bot_task = {
        let file_queue = Arc::clone(&file_queue);
        let permissions = Arc::clone(&permissions);
        let tx = tx.clone();
        let bot = bot.clone();

        spawn(async move {
            teloxide::repl(bot, move |bot: Bot, msg: Message| {
                debug!("Received message: {:?}", msg);

                let bot = Arc::new(bot);
                let bot_clone = Arc::clone(&bot);
                let permissions = Arc::clone(&permissions);
                let file_queue = Arc::clone(&file_queue);
                let tx = tx.clone();

                async move {
                    let permissions = permissions.lock().await;

                    if !permissions.user_has_access(msg.chat.id.to_string(), &msg.from().unwrap().id.to_string()) {
                        info!("User {} does not have access to chat {}",  msg.from().unwrap().id, msg.clone().chat.id);

                        return Ok(());
                    }

                    info!("User {} has access to chat {}", msg.from().unwrap().id, msg.clone().chat.id);

                    if let Err(e) = bot::process_message(bot_clone, msg.clone(), file_queue, tx).await {
                        error!("Failed to process message: {}", e);
                    }

                    Ok(())
                }
            }).await;
        })
    };

    let queue_processor_task = {
        let file_queue: FileQueueType = Arc::clone(&file_queue);
        let bot = Arc::new(bot.clone());

        spawn(async move {
            if let Err(e) = bot::process_queue(bot, file_queue, rx).await {
                error!("Failed to process queue: {}", e);
            }
        })
    };

    let server_task = spawn(async move {
        let app = server::create_app().await;

        let addr: String = format!("0.0.0.0:{}", server_port);
        let listener = TcpListener::bind(&addr).await
            .expect("Failed to bind to address");

        let local_addr = listener.local_addr().unwrap();
        let ip = local_addr.ip().to_string();
        let port = local_addr.port();

        info!("Server is running at http://{}:{}/", ip, port);

        if let Err(e) = axum::serve(listener, app).await {
            error!("Server error: {}", e);
        }
    });

    let ctrl_c_task = {
        spawn(async move {
            signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

            info!("Received Ctrl+C, shutting down...");

            match send_command(&Config::instance().await.pipe_path(), "shutdown").await {
                Ok(_) => info!("Command 'shutdown' sent"),
                Err(_) => error!("Failed to send shutdown command")
            };
        })
    };

    let update_cli_task = {
        let permissions = Arc::clone(&permissions);

        spawn(async move {
            handle_cli(permissions).await;
        })
    };

    tokio::select! {
        _ = bot_task => {},
        _ = queue_processor_task => {},
        _ = server_task => {},
        _ = update_cli_task => {},
        _ = ctrl_c_task => {},
    }

    info!("Shutting down gracefully");

    Ok(())
}
