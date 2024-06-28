mod bot;
mod server;
mod utils;

use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    utils::load_env();

    let server_port = utils::fetch_server_port();

    let bot = bot::get_bot().unwrap();

    let bot_task = tokio::spawn(async move {
        teloxide::repl(bot, |bot: teloxide::Bot, msg: teloxide::types::Message| async move {
            bot::process_message(bot, msg).await.expect("TODO: panic message");
            Ok(())
        })
            .await;
    });

    let server_task = tokio::spawn(async move {
        let app = server::create_app();

        let addr: String = format!("0.0.0.0:{server_port}").parse().unwrap();

        let listener = TcpListener::bind(addr).await.unwrap();

        axum::serve(listener, app).await.unwrap();
    });

    tokio::select! {
        _ = bot_task => {},
        _ = server_task => {},
    }
}
