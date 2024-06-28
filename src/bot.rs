use std::process;
use uuid::Uuid;
use teloxide::{Bot, prelude::*};
use teloxide::net::Download;
use reqwest::Url;
use tokio::fs;
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

pub async fn process_message(bot: Bot, msg: Message) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(document) = msg.document() {
        handle_file(&bot, &msg, document.file.id.clone(), document.file_name.as_ref()).await?;
    } else if let Some(photo) = msg.photo().and_then(|p| p.last()) {
        handle_file(&bot, &msg, photo.file.id.clone(), None).await?;
    } else if let Some(video) = msg.video() {
        handle_file(&bot, &msg, video.file.id.clone(), video.file_name.as_ref()).await?;
    } else if let Some(animation) = msg.animation() {
        handle_file(&bot, &msg, animation.file.id.clone(), animation.file_name.as_ref()).await?;
    }

    Ok(())
}
async fn handle_file(
    bot: &Bot,
    msg: &Message,
    file_id: String,
    file_name: Option<&String>,
) -> Result<(), Box<dyn std::error::Error>> {
    bot.send_message(msg.chat.id, "Starting file downloading").await?;

    println!("{file_id}");

    let file_info = bot.get_file(file_id).await.unwrap();

    let uuid = Uuid::new_v4();

    let final_file_name = match file_name {
        Some(name) => format!("files/{}_{}", uuid, name),
        None => format!("files/{}_{}", uuid, utils::get_file_name_from_path(&file_info.path).unwrap()),
    };

    println!("{}", file_info.path);

    utils::create_directory("files").await.expect("Failed to create directory 'files'");

    let mut dst = fs::File::create(final_file_name.clone()).await?;

    bot.download_file(&utils::get_folder_and_file_name(&file_info.path).unwrap(), &mut dst).await?;

    let final_size = utils::get_file_size(&final_file_name).await?;

    println!("File saved: {:?}", final_file_name);

    bot.send_message(
        msg.chat.id,
        format!(
            "Downloaded. Size: {} bytes\n{}{}",
            final_size.to_string(),
            utils::fetch_domain(),
            final_file_name
        ),
    ).await?;

    Ok(())
}