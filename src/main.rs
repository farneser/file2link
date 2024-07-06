use std::error::Error;
use std::sync::Arc;

use log::{debug, error, info, warn};
use teloxide::Bot;
use teloxide::prelude::Message;
use tokio::{signal, time};
use tokio::net::TcpListener;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};

use crate::bot::FileQueueType;

mod bot;
mod server;
mod utils;
mod chat_config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    utils::load_env();

    pretty_env_logger::init();

    info!("Starting up...");

    let server_port = utils::fetch_server_port();

    info!("Server port: {}", server_port);

    let update_permissions_interval = utils::fetch_update_permissions_interval();

    let raw_permissions = chat_config::load_config()
        .await.expect("Failed to load config");

    let permissions = Arc::new(Mutex::new(raw_permissions));

    let bot = match bot::get_bot() {
        Ok(bot) => { bot }
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
        let app = server::create_app();

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

    let ctrl_c_task = spawn(async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");

        info!("\n\nReceived Ctrl+C, shutting down...");
    });

    let update_permissions_task: tokio::task::JoinHandle<Result<(), String>> = {
        let permissions = Arc::clone(&permissions);

        spawn(async move {
            if update_permissions_interval > 0 {
                let mut interval = time::interval(time::Duration::from_secs(update_permissions_interval.abs() as u64));
                loop {
                    interval.tick().await;

                    let result = tokio::task::spawn_blocking(|| {
                        chat_config::load_config()
                    }).await;

                    match result {
                        Ok(new_permissions) => {
                            let mut permissions = permissions.lock().await;

                            *permissions = new_permissions.await.expect("Failed to load config");

                            info!("Permissions updated successfully");
                        }
                        Err(e) => {
                            error!("Spawn blocking task failed: {}", e);
                        }
                    }
                }
            } else {
                warn!("Permissions update interval is set to 0, permissions will not be updated automatically");

                loop {
                    // FIXME 06.07.2024: this shitpost is here because i'm do not know how to run this task by condition

                    time::sleep(time::Duration::from_secs(10)).await;
                }
            }
        })
    };

    tokio::select! {
        _ = bot_task => {},
        _ = queue_processor_task => {},
        _ = server_task => {},
        _ = update_permissions_task => {}
        _ = ctrl_c_task => {},
    }

    Ok(())
}
