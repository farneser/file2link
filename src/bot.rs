use std::process;
use std::sync::Arc;
use uuid::Uuid;
use teloxide::{Bot, prelude::*};
use teloxide::net::Download;
use reqwest::Url;
use teloxide::types::File;
use tokio::{fs, spawn};
use crate::utils;

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

pub async fn process_message(bot: Arc<Bot>, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
    let msg_copy = Arc::new(msg);

    if let Some(document) = msg_copy.clone().document() {
        handle_file(bot.clone(), msg_copy, document.file.id.clone(), document.clone().file_name).await?;
    } else if let Some(photo) = msg_copy.clone().photo().and_then(|p| p.last()) {
        handle_file(bot.clone(), msg_copy.clone(), photo.file.id.clone(), None).await?;
    } else if let Some(video) = msg_copy.clone().video() {
        handle_file(bot.clone(), msg_copy.clone(), video.file.id.clone(), video.clone().file_name).await?;
    } else if let Some(animation) = msg_copy.clone().animation() {
        handle_file(bot.clone(), msg_copy.clone(), animation.file.id.clone(), animation.clone().file_name).await?;
    }

    Ok(())
}

async fn handle_file(
    bot: Arc<Bot>,
    msg: Arc<Message>,
    file_id: String,
    file_name: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    bot.send_message(msg.chat.id, "Starting file downloading").await?;

    println!("{file_id}");

    utils::create_directory("files").await.expect("Failed to create directory 'files'");

    let bot_clone = Arc::clone(&bot);

    let msg_clone_for_spawn = Arc::clone(&msg);

    spawn(async move {
        let file_info = bot_clone.clone().get_file(file_id).await.unwrap();

        let uuid = Uuid::new_v4();

        let final_file_name = match file_name {
            Some(name) => format!("files/{}_{}", uuid, name),
            None => format!("files/{}_{}", uuid, utils::get_file_name_from_path(&file_info.path).unwrap()),
        };

        println!("{}", &file_info.path);

        if let Err(e) = download_file(&bot_clone, msg_clone_for_spawn.clone(), final_file_name, file_info.clone()).await {
            eprintln!("Failed to download file: {}", e);
        }
    });

    Ok(())
}

async fn download_file(bot: &Bot, msg: Arc<Message>, final_file_name: String, file_info: File) -> Result<String, String> {
    match fs::File::create(&final_file_name).await {
        Ok(mut dst) => {
            if let Ok(_) = bot.download_file(&utils::get_folder_and_file_name(&file_info.path).unwrap(), &mut dst).await {
                let final_size = utils::get_file_size(&final_file_name).await.unwrap_or(0);

                println!("File saved: {:?}", final_file_name);

                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Downloaded. Size: {} bytes\n\n{}{}",
                        final_size,
                        utils::fetch_domain(),
                        final_file_name
                    ),
                ).await.expect("Failed to send message");

                Ok("File downloaded".to_owned())
            } else {
                println!("Failed to download the file.");
                Err("Failed to download the file".to_owned())
            }
        }
        Err(e) => {
            println!("Failed to create file: {:?}", e);
            Err("Failed to create file".to_owned())
        }
    }
}
