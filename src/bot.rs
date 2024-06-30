use std::process;
use std::sync::Arc;

use reqwest::Url;
use teloxide::{Bot, prelude::*};
use teloxide::net::Download;
use teloxide::types::ParseMode;
use tokio::fs;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::utils;

pub type FileQueueType = Arc<Mutex<Vec<(Arc<Message>, String, Option<String>)>>>;

pub fn get_bot() -> Option<Bot> {
    let token = match utils::fetch_bot_token() {
        Ok(token) => token,
        Err(e) => {
            eprintln!("Failed to fetch bot token: {}", e);
            process::exit(1);
        }
    };

    let api_url = utils::fetch_telegram_api();

    let url = match Url::parse(&api_url) {
        Ok(url) => Some(url),
        Err(e) => {
            eprintln!("Failed to parse API_URL: {}", e);
            return None;
        }
    };

    let mut bot = Bot::new(token);

    if let Some(url) = url {
        println!("{}", url.to_string().to_owned());
        bot = bot.set_api_url(url);
    }

    Some(bot)
}

pub async fn process_message(
    bot: Arc<Bot>,
    msg: Message,
    file_queue: FileQueueType,
    tx: Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    let msg_copy = Arc::new(msg);

    if let Some(document) = msg_copy.clone().document() {
        handle_file(bot.clone(), msg_copy, document.file.id.clone(), document.clone().file_name, file_queue, tx).await?;
    } else if let Some(photo) = msg_copy.clone().photo().and_then(|p| p.last()) {
        handle_file(bot.clone(), msg_copy.clone(), photo.file.id.clone(), None, file_queue, tx).await?;
    } else if let Some(video) = msg_copy.clone().video() {
        handle_file(bot.clone(), msg_copy.clone(), video.file.id.clone(), video.clone().file_name, file_queue, tx).await?;
    } else if let Some(animation) = msg_copy.clone().animation() {
        handle_file(bot.clone(), msg_copy.clone(), animation.file.id.clone(), animation.clone().file_name, file_queue, tx).await?;
    }

    Ok(())
}

async fn handle_file(
    bot: Arc<Bot>,
    msg: Arc<Message>,
    file_id: String,
    file_name: Option<String>,
    file_queue: FileQueueType,
    tx: Sender<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    {
        let mut queue = file_queue.lock().await;

        queue.push((msg.clone(), file_id.clone(), file_name.clone()));

        let position = queue.len();

        bot.send_message(msg.chat.id, format!("Queue position: {}", position))
            .reply_to_message_id(msg.id).await?;
    }

    tx.send(()).await?;

    Ok(())
}

pub(crate) async fn process_queue(
    bot: Arc<Bot>,
    file_queue: FileQueueType,
    mut rx: Receiver<()>,
) {
    while let Some(()) = rx.recv().await {
        let (msg, file_id, file_name) = {
            let queue = file_queue.lock().await;

            if let Some(item) = queue.first() {
                item.clone()
            } else {
                continue;
            }
        };

        if let Err(e) = download_and_process_file(bot.clone(), msg.clone(), file_id.clone(), file_name.clone()).await {
            eprintln!("Failed to process file: {}", e);
        }

        let mut queue = file_queue.lock().await;

        queue.remove(0);
    }
}

async fn download_and_process_file(
    bot: Arc<Bot>,
    msg: Arc<Message>,
    file_id: String,
    file_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    bot.send_message(msg.chat.id, "Starting file downloading")
        .reply_to_message_id(msg.id).await?;

    utils::create_directory("files").await.expect("Failed to create directory 'files'");

    let file_info = bot.clone().get_file(file_id).await.unwrap();

    let uuid = Uuid::new_v4();

    let name = file_name.map(|name| {
        let name = name.to_string();
        name.replace(' ', "_")
    });

    let final_file_name = match name {
        Some(name) => format!("files/{}_{}", uuid, name),
        None => format!("files/{}_{}", uuid, utils::get_file_name_from_path(&file_info.path).unwrap()),
    };

    println!("{}", &file_info.path);

    match fs::File::create(&final_file_name).await {
        Ok(mut dst) => {
            if let Ok(_) = bot.download_file(&utils::get_folder_and_file_name(&file_info.path).unwrap(), &mut dst).await {
                let final_size = utils::get_file_size(&final_file_name).await.unwrap_or(0);

                println!("File saved: {:?}", final_file_name);

                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Downloaded. Size: {} bytes\n\n<b><a href=\"{}{}\">{}{}</a></b>",
                        final_size,
                        utils::fetch_domain(),
                        final_file_name,
                        utils::fetch_domain(),
                        final_file_name
                    ),
                ).parse_mode(ParseMode::Html)
                    .reply_to_message_id(msg.id).await
                    .expect("Failed to send message");

                Ok(())
            } else {
                println!("Failed to download the file.");
                Err("Failed to download the file".to_owned().into())
            }
        }
        Err(e) => {
            println!("Failed to create file: {:?}", e);
            Err("Failed to create file".to_owned().into())
        }
    }
}