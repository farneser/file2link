use crate::bot::TeloxideBot;
use futures::{Stream, StreamExt};
use log::{debug, error, info, warn};
use nanoid::nanoid;
use shared::config::Config;
use shared::utils;
use std::error::Error;
use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;
use teloxide::net::Download;
use teloxide::payloads::EditMessageTextSetters;
use teloxide::prelude::{Message, Requester};
use teloxide::types::ParseMode;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc::Receiver;
use tokio::sync::Mutex;
use tokio::time::{interval, sleep};
use tokio_util::bytes::Bytes;

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


pub async fn process_queue(
    bot: Arc<TeloxideBot>,
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
            match bot.get_teloxide_bot().edit_message_text(
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

            bot.get_teloxide_bot().edit_message_text(
                queue_item.queue_message.chat.id,
                queue_item.queue_message.id,
                format!("File processed. Remaining files in queue: {}", queue.len()),
            ).await.expect("Failed to edit message");
        }

        info!("Removed item from queue. Remaining items in queue: {}", queue.len());
    })
}


async fn download_and_process_file_from_telegram(
    bot: Arc<TeloxideBot>,
    queue_item: FileQueueItem,
    file_id: &String,
) -> Result<(), String> {
    info!("Starting download for file ID: {}", file_id);

    let (file_path, file_size) = get_file_info(bot.clone(), file_id)
        .await.map_err(|_| "Failed to get file info".to_owned())?;
    info!("File path obtained: {}", &file_path);

    let final_file_name = generate_final_file_name(&queue_item, &file_path).await;

    let stream = bot.get_teloxide_bot()
        .download_file_stream(&utils::get_folder_and_file_name(&file_path).unwrap());

    let downloaded_size = create_and_save_file(
        bot.clone(),
        &final_file_name,
        stream,
        Some(file_size),
    ).await?;

    edit_message_with_file_link(bot, &queue_item, &final_file_name, downloaded_size).await
}

async fn download_and_process_file_from_url(
    bot: Arc<TeloxideBot>,
    queue_item: FileQueueItem,
    url: &String,
) -> Result<(), String> {
    info!("Starting download from URL: {}", url);

    let response = reqwest::get(url).await.map_err(|e| format!("Failed to download file: {}", e))?;

    let content_disposition = response.headers().get(reqwest::header::CONTENT_DISPOSITION);
    let file_name = content_disposition
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split("filename=").nth(1))
        .map(|v| v.trim_matches('"').to_string())
        .or_else(|| url.split('/').last().map(|name| name.to_string()))
        .filter(|name| !name.is_empty())
        .ok_or("Could not determine file name")?;

    let final_file_name = generate_final_file_name(&queue_item, &file_name).await;

    let stream = response.bytes_stream();
    let downloaded_size = create_and_save_file(bot.clone(), &final_file_name, stream, None).await?;

    edit_message_with_file_link(bot, &queue_item, &final_file_name, downloaded_size).await
}


async fn generate_final_file_name(queue_item: &FileQueueItem, file_path_or_name: &str) -> String {
    let id = nanoid!(5);
    let name = queue_item.file_name.as_ref().map(|name| name.to_string().replace(' ', "_"));
    match name {
        Some(name) => format!("{}_{}", id, name),
        None => format!("{}_{}", id, utils::get_file_name_from_path(&file_path_or_name).unwrap()),
    }
}

/// Get file info from Telegram
///
/// # Arguments
/// * `bot` - Bot instance
/// * `id` - File ID
/// # Returns
/// * `Result` containing a tuple of file path and file size
/// * `String` containing an error message
async fn get_file_info(bot: Arc<TeloxideBot>, id: &String) -> Result<(String, u32), String> {
    const MAX_ATTEMPTS: u32 = 3;

    for attempt in 1..=MAX_ATTEMPTS {
        match bot.get_teloxide_bot().get_file(id).await {
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

// #[derive(BotCommands, Clone)]
// #[command(rename_rule = "lowercase", description = "These commands are supported:")]
// enum Command {
//     #[command(description = "display this text.")]
//     Help,
//     #[command(description = "download a file from the URL.")]
//     Url(String),
// }
//

async fn edit_message_with_file_link(
    bot: Arc<TeloxideBot>,
    queue_item: &FileQueueItem,
    file_name: &str,
    file_size: u32,
) -> Result<(), String> {
    let file_domain = Config::instance().await.file_domain();
    let edit_result = bot.get_teloxide_bot().edit_message_text(
        queue_item.message.chat.id,
        queue_item.queue_message.id,
        format!(
            "Downloaded. Size: {} bytes\n\n<b><a href=\"{}{}\">{}{}</a></b>",
            file_size,
            file_domain,
            file_name,
            file_domain,
            file_name
        ),
    )
        .parse_mode(ParseMode::Html)
        .await;

    if edit_result.is_err() {
        error!("Failed to edit message");
        return Err("Failed to edit message".to_owned());
    }

    Ok(())
}

async fn create_and_save_file(
    _bot: Arc<TeloxideBot>,
    file_name: &str,
    mut stream: impl Stream<Item=Result<Bytes, reqwest::Error>> + Unpin,
    total_size: Option<u32>,
) -> Result<u32, String> {
    utils::create_directory("files")
        .await.map_err(|e| format!("Failed to create directory 'files': {}", e))?;

    let file_name_with_folder = format!("files/{}", file_name);
    let mut dst = File::create(&file_name_with_folder)
        .await.map_err(|e| format!("Failed to create file: {:?}", e))?;

    let mut total_bytes = 0u32;
    let mut interval = interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        total_bytes += bytes.len() as u32;
                        dst.write_all(&bytes).await.map_err(|e| e.to_string())?;
                    }
                    Some(Err(e)) => {
                        warn!("Error: {}", e);
                        return Err("Failed to download the file".to_owned());
                    }
                    None => break,
                }
            }
            _ = interval.tick() => {
                if let Some(size) = total_size {
                    info!("Downloaded {} of {} bytes", total_bytes, size);
                } else {
                    info!("Downloaded {} bytes", total_bytes);
                }
            }
        }
    }

    Ok(total_bytes)
}
