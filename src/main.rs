use bot::process_queue;
use std::error::Error;
use std::sync::Arc;

use bot::bot::{Bot as BotTrait, TeloxideBot};
use bot::FileQueueType;
use cli::utils::send_command;
use log::{error, info};
use tokio::net::TcpListener;
use tokio::signal;
use tokio::spawn;
use tokio::sync::{mpsc, Mutex};

mod server;
use core::chat_config;
use core::config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    config::load_env();

    pretty_env_logger::init();

    info!("Starting up...");

    let server_port = config::Config::instance().await.server_port();
    info!("Server port: {}", server_port);

    let raw_permissions = chat_config::load_config()
        .await.expect("Failed to load config");

    let permissions = Arc::new(Mutex::new(raw_permissions));

    let bot = TeloxideBot::new(config::Config::instance().await, permissions.clone(), Arc::new(Mutex::new(Vec::new())));

    let bot_clone = Arc::new(bot);

    let file_queue: FileQueueType = Arc::new(Mutex::new(Vec::new()));

    let (tx, rx) = mpsc::channel(100);

    let bot_task = {
        let tx = tx.clone();
        let bot = Arc::clone(&bot_clone);

        spawn(async move {
            bot.run(tx).await;
        })
    };

    let queue_processor_task = {
        let file_queue: FileQueueType = Arc::clone(&file_queue);

        let bot = Arc::clone(&bot_clone);

        spawn(async move {
            if let Err(e) = process_queue(bot, file_queue, rx).await {
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

            match send_command(&config::Config::instance().await.pipe_path(), "shutdown").await {
                Ok(_) => info!("Command 'shutdown' sent"),
                Err(_) => error!("Failed to send shutdown command")
            };
        })
    };

    let update_cli_task = {
        let permissions = Arc::clone(&permissions);

        spawn(async move {
            core::cli_utils::handle_cli(permissions).await;
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
