use std::error::Error;
use std::process;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use log::{debug, error, info, warn};
use reqwest::{Client, Url};
use teloxide::{Bot, prelude::*};
use teloxide::net::Download;
use teloxide::types::ParseMode;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};
use uuid::Uuid;

use crate::utils;

#[derive(Debug, Clone)]
pub struct FileQueueItem {
    message: Arc<Message>,
    queue_message: Arc<Message>,
    file_id: String,
    file_name: Option<String>,
    file_size: u64,
}

impl FileQueueItem {
    pub fn new(
        message: Arc<Message>,
        queue_message: Arc<Message>,
        file_id: String,
        file_name: Option<String>,
        file_size: u64,
    ) -> Self {
        Self {
            message,
            queue_message,
            file_id,
            file_name,
            file_size,
        }
    }
}

pub type FileQueueType = Arc<Mutex<Vec<FileQueueItem>>>;

pub fn get_bot() -> Result<Bot, String> {
    let token = match utils::fetch_bot_token() {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to fetch bot token: {}", e);

            process::exit(1);
        }
    };

    let api_url = utils::fetch_telegram_api();

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

    async fn process_file(
        bot: Arc<Bot>,
        msg_copy: Arc<Message>,
        file_id: String,
        file_name: Option<String>,
        file_size: u64,
        file_queue: FileQueueType,
        tx: Sender<()>,
    ) -> Result<(), Box<dyn Error>> {
        handle_file(bot, msg_copy, file_id, file_name, file_size, file_queue, tx)
            .await.expect("Failed to handle file");
        Ok(())
    }

    let file_info = if let Some(document) = msg_copy.document() {
        info!("Processing document file with ID: {}", document.file.id);

        Some((document.file.id.clone(), document.file_name.clone(), document.file.size as u64))
    } else if let Some(photo) = msg_copy.photo().and_then(|p| p.last()) {
        info!("Processing photo file with ID: {}", photo.file.id);

        Some((photo.file.id.clone(), None, photo.file.size as u64))
    } else if let Some(video) = msg_copy.video() {
        info!("Processing video file with ID: {}", video.file.id);

        Some((video.file.id.clone(), video.file_name.clone(), video.file.size as u64))
    } else if let Some(animation) = msg_copy.animation() {
        info!("Processing animation file with ID: {}", animation.file.id);

        Some((animation.file.id.clone(), animation.file_name.clone(), animation.file.size as u64))
    } else {
        None
    };

    if let Some((file_id, file_name, file_size)) = file_info {
        process_file(
            bot.clone(),
            msg_copy.clone(),
            file_id,
            file_name,
            file_size,
            file_queue,
            tx,
        ).await.expect("Failed to process file");
    } else {
        debug!("Received a non-file message");
    }

    Ok(())
}

async fn handle_file(
    bot: Arc<Bot>,
    msg: Arc<Message>,
    file_id: String,
    file_name: Option<String>,
    file_size: u64,
    file_queue: FileQueueType,
    tx: Sender<()>,
) -> Result<(), Box<dyn Error>> {
    {
        let mut queue = file_queue.lock().await;

        let position = queue.len() + 1;

        let queue_message = bot.send_message(msg.chat.id, format!("Queue position: {}", position))
            .reply_to_message_id(msg.id)
            .await.expect("Failed to send message");

        let queue_message_clone = Arc::new(queue_message);

        queue.push(FileQueueItem::new(msg.clone(), queue_message_clone, file_id.clone(), file_name.clone(), file_size));

        info!("Added file with ID {} to queue. Current queue position: {}", file_id, position);
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

        if let Err(e) = download_and_process_file(
            bot.clone(),
            queue_item.clone(),
        ).await {
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

        info!("Removed file from queue. Remaining files in queue: {}", queue.len());
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

async fn download_and_process_file(
    bot: Arc<Bot>,
    queue_item: FileQueueItem,
) -> Result<(), String> {
    info!("Starting download for file ID: {}", queue_item.file_id);

    utils::create_directory("files")
        .await.expect("Failed to create directory 'files'");

    let (file_path, file_size) = match get_file_info(bot.clone(), &queue_item.file_id).await {
        Ok(info) => { info }
        Err(_) => { return Err("Failed to get file info".to_owned()); }
    };

    info!("File path obtained: {}", &file_path);

    let uuid = Uuid::new_v4();

    let name = queue_item.file_name.map(|name| {
        let name = name.to_string();

        name.replace(' ', "_")
    });

    let final_file_name = match name {
        Some(name) => format!("files/{}_{}", uuid, name),
        None => format!("files/{}_{}", uuid, utils::get_file_name_from_path(&file_path).unwrap()),
    };

    debug!("File path obtained: {}", &file_path);

    match File::create(&final_file_name).await {
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

            let file_size = utils::get_file_size(&final_file_name).await.unwrap_or(file_size as u64);

            info!("File saved: {:?}", final_file_name);

            let edit_result = bot.edit_message_text(
                queue_item.message.chat.id,
                queue_item.queue_message.id,
                format!(
                    "Downloaded. Size: {} bytes\n\n<b><a href=\"{}{}\">{}{}</a></b>",
                    file_size,
                    utils::fetch_domain(),
                    final_file_name,
                    utils::fetch_domain(),
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
