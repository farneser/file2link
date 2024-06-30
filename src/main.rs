mod bot;
mod server;
mod utils;

use std::sync::Arc;
use teloxide::Bot;
use teloxide::prelude::Message;
use tokio::net::TcpListener;
use tokio::spawn;
use tokio::signal;
use tokio::sync::{mpsc, Mutex};
use crate::bot::FileQueueType;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    utils::load_env();

    let server_port = utils::fetch_server_port();

    let bot = bot::get_bot().unwrap();

    let file_queue: FileQueueType = Arc::new(Mutex::new(Vec::<(Arc<Message>, String, Option<String>)>::new()));

    let (tx, rx) = mpsc::channel(1);
    tx.send(()).await.unwrap();

    let bot_task = {
        let file_queue = Arc::clone(&file_queue);
        let tx = tx.clone();
        let bot = bot.clone();

        spawn(async move {
            teloxide::repl(bot, move |bot: Bot, msg: Message| {
                let bot = Arc::new(bot);
                let bot_clone = Arc::clone(&bot);
                let file_queue = Arc::clone(&file_queue);
                let tx = tx.clone();

                async move {
                    bot::process_message(bot_clone, msg, file_queue, tx).await.expect("Fail: process message");
                    Ok(())
                }
            })
                .await;
        })
    };

    let queue_processor_task = {
        let file_queue: FileQueueType = Arc::clone(&file_queue);
        let bot = Arc::new(bot.clone());

        spawn(async move {
            bot::process_queue(bot, file_queue, rx).await;
        })
    };

    let server_task = spawn(async move {
        let app = server::create_app();

        let addr: String = format!("0.0.0.0:{}", server_port);
        let listener = TcpListener::bind(&addr).await.unwrap();

        axum::serve(listener, app).await.unwrap();
    });

    let ctrl_c_task = spawn(async {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        println!("\n\nReceived Ctrl+C, shutting down...");
    });

    tokio::select! {
        _ = bot_task => {},
        _ = queue_processor_task => {},
        _ = server_task => {},
        _ = ctrl_c_task => {},
    }
}
