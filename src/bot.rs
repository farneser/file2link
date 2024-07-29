use std::error::Error;
use std::fmt::Display;
use std::process;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use log::{debug, error, info, warn};
use nanoid::nanoid;
use reqwest::{Client, Url};
use teloxide::{Bot, prelude::*};
use teloxide::net::Download;
use teloxide::types::ParseMode;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};

use crate::config::Config;
use crate::utils;

#[derive(Debug, Clone)]
pub struct FileQueueItem {
    message: Arc<Message>,
    queue_message: Arc<Message>,
    file_id: Option<String>,
    file_name: Option<String>,
    url: Option<String>,
}

impl FileQueueItem {
    pub fn new(
        message: Arc<Message>,
        queue_message: Arc<Message>,
        file_id: Option<String>,
        file_name: Option<String>,
        url: Option<String>,
    ) -> Self {
        Self {
            message,
            queue_message,
            file_id,
            file_name,
            url,
        }
    }
}

impl Display for FileQueueItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileQueueItem {{ message: {:?}, queue_message: {:?}, file_id: {:?}, file_name: {:?}, url: {:?} }}", self.message, self.queue_message, self.file_id, self.file_name, self.url)
    }
}

pub type FileQueueType = Arc<Mutex<Vec<FileQueueItem>>>;

pub async fn get_bot() -> Result<Bot, String> {
    let token = match Config::instance().await.bot_token() {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to fetch bot token: {}", e);

            process::exit(1);
        }
    };

    let api_url = Config::instance().await.telegram_api_url();

    let url = match Url::parse(&api_url) {
        Ok(url) => Some(url),
        Err(e) => {
            error!("Failed to parse API_URL: {}", e);

            return Err("Failed to parse API_URL".to_owned());
        }
    };

    let client = match Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(300))
        .tcp_nodelay(true)
        .build() {
        Ok(client) => client,
        Err(e) => {
            error!("Failed to create client: {}", e);

            return Err("Failed to create client".to_owned());
        }
    };

    let mut bot = Bot::with_client(token, client);

    if let Some(url) = url {
        info!("API URL: {}", url.to_string().to_owned());

        bot = bot.set_api_url(url);
    }

    Ok(bot)
}

pub async fn process_message(
    bot: Arc<Bot>,
    msg: Message,
    file_queue: FileQueueType,
    tx: Sender<()>,
) -> Result<(), Box<dyn Error>> {
    let msg_copy = Arc::new(msg.clone());

    let file_info = if let Some(document) = msg_copy.document() {
        info!("Processing document file with ID: {}", document.file.id);

        Some((Some(document.file.id.clone()), document.file_name.clone(), None))
    } else if let Some(photo) = msg_copy.photo().and_then(|p| p.last()) {
        info!("Processing photo file with ID: {}", photo.file.id);

        Some((Some(photo.file.id.clone()), None, None))
    } else if let Some(video) = msg_copy.video() {
        info!("Processing video file with ID: {}", video.file.id);

        Some((Some(video.file.id.clone()), video.file_name.clone(), None))
    } else if let Some(animation) = msg_copy.animation() {
        info!("Processing animation file with ID: {}", animation.file.id);

        Some((Some(animation.file.id.clone()), animation.file_name.clone(), None))
    } else if let Some(text) = msg_copy.text() {
        if text.starts_with("/url") {
            let url_text = &text[5..].to_string();

            info!("Processing URL: {}", url_text);

            if let Ok(url) = Url::parse(url_text) {
                info!("Processing URL: {}", url);

                // answer_commands(bot.clone(), msg.clone(), Command::Url(url.to_string()), &tx).await.expect("Failed to answer command");

                Some((None, None, Some(url.to_string())))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((file_id, file_name, url)) = file_info {
        handle_file(
            bot.clone(),
            msg_copy.clone(),
            file_id,
            file_name,
            url,
            file_queue,
            &tx,
        ).await.expect("Failed to process file");
    } else {
        debug!("Received a non-file message");
    }

    Ok(())
}

async fn handle_file(
    bot: Arc<Bot>,
    msg: Arc<Message>,
    file_id: Option<String>,
    file_name: Option<String>,
    url: Option<String>,
    file_queue: FileQueueType,
    tx: &Sender<()>,
) -> Result<(), Box<dyn Error>> {
    {
        let mut queue = file_queue.lock().await;

        let position = queue.len() + 1;

        let queue_message = bot.send_message(msg.chat.id, format!("Queue position: {}", position))
            .reply_to_message_id(msg.id)
            .await.expect("Failed to send message");

        let queue_message_clone = Arc::new(queue_message);

        queue.push(FileQueueItem::new(msg.clone(), queue_message_clone, file_id.clone(), file_name.clone(), url.clone()));

        info!("Added item to queue. Current queue position: {}", position);
    }

    tx.send(()).await?;

    Ok(())
}

pub async fn process_queue(
    bot: Arc<Bot>,
    file_queue: FileQueueType,
    mut rx: Receiver<()>,
) -> Result<(), Box<dyn Error>> {
    Ok(while let Some(()) = rx.recv().await {
        let queue_item = {
            let queue = file_queue.lock().await;

            if let Some(item) = queue.first() {
                item.clone()
            } else {
                continue;
            }
        };

        debug!("Processing file: {:?}", queue_item);

        const MAX_ATTEMPTS: u32 = 3;

        for attempt in 1..=MAX_ATTEMPTS {
            match bot.edit_message_text(
                queue_item.message.chat.id,
                queue_item.queue_message.id,
                "Processing file...",
            ).await {
                Ok(_) => break,
                Err(e) => {
                    if attempt == MAX_ATTEMPTS {
                        warn!("Failed to edit message text after {} attempts: {:?}", MAX_ATTEMPTS, e);
                    } else {
                        let delay = Duration::from_secs(2_u64.pow(attempt - 1));

                        warn!("Attempt to edit message {} failed, retrying in {:?}... Error: {:?}", attempt, delay, e);

                        sleep(delay).await;
                    }
                }
            }
        }

        if let Err(e) = if let Some(url) = &queue_item.url {
            download_and_process_file_from_url(
                bot.clone(),
                queue_item.clone(),
                url,
            ).await
        } else if let Some(file_id) = &queue_item.file_id {
            download_and_process_file_from_telegram(
                bot.clone(),
                queue_item.clone(),
                file_id,
            ).await
        } else {
            Err("No file_id or url found".to_string())
        } {
            error!("Failed to process file: {}", e);
            continue;
        }

        let mut queue = file_queue.lock().await;

        queue.remove(0);

        if let Some(front) = queue.first() {
            let queue_item = front.clone();

            bot.edit_message_text(
                queue_item.queue_message.chat.id,
                queue_item.queue_message.id,
                format!("File processed. Remaining files in queue: {}", queue.len()),
            ).await.expect("Failed to edit message");
        }

        info!("Removed item from queue. Remaining items in queue: {}", queue.len());
    })
}

/// Get file info from Telegram
///
/// # Arguments
/// * `bot` - Bot instance
/// * `id` - File ID
/// # Returns
/// * `Result` containing a tuple of file path and file size
/// * `String` containing an error message
async fn get_file_info(bot: Arc<Bot>, id: &String) -> Result<(String, u32), String> {
    const MAX_ATTEMPTS: u32 = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match bot.clone().get_file(id).await {
            Ok(info) => return Ok((info.clone().path, info.size)),
            Err(e) => {
                if attempt == MAX_ATTEMPTS {
                    error!("Failed to get file info after {} attempts: {:?}", MAX_ATTEMPTS, e);

                    return Err("Failed to get file info".to_owned());
                } else {
                    warn!("Attempt {} failed, retrying... Error: {:?}", attempt, e);

                    sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    unreachable!()
}

async fn download_and_process_file_from_telegram(
    bot: Arc<Bot>,
    queue_item: FileQueueItem,
    file_id: &String,
) -> Result<(), String> {
    info!("Starting download for file ID: {}", file_id);

    utils::create_directory("files")
        .await.expect("Failed to create directory 'files'");

    let (file_path, file_size) = match get_file_info(bot.clone(), file_id).await {
        Ok(info) => { info }
        Err(_) => { return Err("Failed to get file info".to_owned()); }
    };

    info!("File path obtained: {}", &file_path);

    let id = nanoid!(5);

    let name = queue_item.file_name.map(|name| {
        let name = name.to_string();

        name.replace(' ', "_")
    });

    let final_file_name = match name {
        Some(name) => format!("{}_{}", id, name),
        None => format!("{}_{}", id, utils::get_file_name_from_path(&file_path).unwrap()),
    };

    let file_name_with_folder = format!("files/{}", final_file_name);

    debug!("File path obtained: {}", &file_path);

    match File::create(&file_name_with_folder).await {
        Ok(mut dst) => {
            let mut stream = bot.download_file_stream(&utils::get_folder_and_file_name(&file_path).unwrap());

            let mut interval = interval(Duration::from_secs(2));
            let mut total_bytes = 0;

            loop {
                tokio::select! {
                    chunk = stream.next() => {
                        match chunk {
                            Some(Ok(bytes)) => {
                                total_bytes += bytes.len();
                                dst.write_all(&bytes).await.map_err(|e| e.to_string())?;
                            }
                            Some(Err(e)) => {
                                warn!("Error: {}", e);

                                return Err("Failed to download the file".to_owned().into());
                            }
                            None => break,
                        }
                    }
                    _ = interval.tick() => {
                        info!("Downloaded {} of {} bytes", total_bytes, file_size);
                    }
                }
            }

            let file_size = utils::get_file_size(&file_name_with_folder).await.unwrap_or(file_size as u64);

            info!("File saved: {:?}", final_file_name);

            let file_domain = Config::instance().await.file_domain();

            let edit_result = bot.edit_message_text(
                queue_item.message.chat.id,
                queue_item.queue_message.id,
                format!(
                    "Downloaded. Size: {} bytes\n\n<b><a href=\"{}{}\">{}{}</a></b>",
                    file_size,
                    file_domain,
                    final_file_name,
                    file_domain,
                    final_file_name
                ),
            ).parse_mode(ParseMode::Html).await;

            match edit_result {
                Ok(_) => {
                    info!("File processed successfully");
                }
                Err(_) => {
                    error!("Failed to edit message");
                    return Err("Failed to edit message".to_owned().into());
                }
            }

            Ok(())
        }

        Err(e) => {
            error!("Failed to create file: {:?}", e);

            Err("Failed to create file".to_owned().into())
        }
    }
}

async fn download_and_process_file_from_url(
    bot: Arc<Bot>,
    queue_item: FileQueueItem,
    url: &String,
) -> Result<(), String> {
    info!("Starting download from URL: {}", url);

    if let Err(e) = utils::create_directory("files").await {
        error!("Failed to create directory 'files': {}", e);
        return Err("Failed to create directory 'files'".to_owned());
    }

    let response = reqwest::get(url).await.map_err(|e| {
        error!("Failed to download file: {}", e);
        "Failed to download file".to_owned()
    })?;

    let content_disposition = response.headers().get(reqwest::header::CONTENT_DISPOSITION);

    let file_name = if let Some(disposition) = content_disposition {
        disposition.to_str().ok()
            .and_then(|v| v.split("filename=").nth(1))
            .map(|v| v.trim_matches('"').to_string())
    } else {
        url.split('/').last().map(|name| name.to_string())
    };

    let file_name = match file_name {
        Some(name) if !name.is_empty() => name,
        _ => {
            error!("Could not determine file name");
            return Err("Could not determine file name".to_owned());
        }
    };

    let id = nanoid!(5);
    let final_file_name = format!("files/{}_{}", id, file_name);

    match File::create(&final_file_name).await {
        Ok(mut dst) => {
            let mut stream = response.bytes_stream();
            let mut total_bytes = 0;

            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(bytes) => {
                        total_bytes += bytes.len();
                        dst.write_all(&bytes).await.map_err(|e| e.to_string())?;
                    }
                    Err(e) => {
                        warn!("Error: {}", e);
                        return Err("Failed to download the file".to_owned());
                    }
                }
            }

            info!("File saved: {:?}", final_file_name);

            let file_domain = Config::instance().await.file_domain();

            let edit_result = bot.edit_message_text(
                queue_item.message.chat.id,
                queue_item.queue_message.id,
                format!(
                    "Downloaded. Size: {} bytes\n\n<b><a href=\"{}{}\">{}{}</a></b>",
                    total_bytes,
                    file_domain,
                    final_file_name,
                    file_domain,
                    final_file_name
                ),
            ).parse_mode(ParseMode::Html).await;

            if edit_result.is_err() {
                error!("Failed to edit message");
                return Err("Failed to edit message".to_owned());
            }

            info!("File processed successfully");
            Ok(())
        }
        Err(e) => {
            error!("Failed to create file: {:?}", e);
            Err("Failed to create file".to_owned())
        }
    }
}

// #[derive(BotCommands, Clone)]
// #[command(rename_rule = "lowercase", description = "These commands are supported:")]
// enum Command {
//     #[command(description = "display this text.")]
//     Help,
//     #[command(description = "download a file from the URL.")]
//     Url(String),
// }
//
// pub async fn answer_commands(bot: Arc<Bot>, msg: Message, cmd: Command, tx: &Sender<()>) -> Result<(), String> {
//     match cmd {
//         Command::Help => {
//             match bot.send_message(msg.chat.id, Command::descriptions().to_string()).await {
//                 Ok(_) => {
//                     info!("Sent help message");
//                 }
//                 Err(_) => {
//                     error!("Failed to send help message");
//
//                     return Err("Failed to send help message".to_owned());
//                 }
//             }
//         }
//         Command::Url(url) => {
//             info!("Processing URL: {}", url.clone().to_string());
//
//             match handle_file(bot.clone(), Arc::new(msg.clone()), Some(url.to_string()), None, None, Arc::new(Default::default()), tx).await {
//                 Ok(_) => {
//                     info!("Processed URL: {}", url.clone().to_string());
//                 }
//                 Err(_) => {
//                     error!("Failed to process URL: {}", url.clone().to_string());
//
//                     return Err("Failed to process URL".to_owned());
//                 }
//             };
//         }
//     };
//
//     Ok(())
// }
