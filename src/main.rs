mod bot;
mod server;
mod utils;

use std::sync::Arc;
use teloxide::Bot;
use teloxide::prelude::Message;
use tokio::net::TcpListener;
use tokio::spawn;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    utils::load_env();

    let server_port = utils::fetch_server_port();

    let bot = bot::get_bot().unwrap();
    let bot_arc = Arc::new(bot.clone());

    let bot_task = spawn(async move {
        teloxide::repl(bot_arc, |bot: Bot, msg: Message| async move {
            bot::process_message(Arc::new(bot), msg).await.expect("TODO: panic message");
            Ok(())
        })
            .await;
    });

    let server_task = spawn(async move {
        let app = server::create_app();

        let addr: String = format!("0.0.0.0:{}", server_port);
        let listener = TcpListener::bind(&addr).await.unwrap();

        axum::serve(listener, app).await.unwrap();
    });

    tokio::select! {
        _ = bot_task => {},
        _ = server_task => {},
    }
}
