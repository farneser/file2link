use std::error::Error;
use std::sync::Arc;

use crate::queue::{FileQueueItem, FileQueueType};
use log::{debug, info};
use regex::Regex;
use teloxide::payloads::SendMessageSetters;
use teloxide::prelude::{Message, Requester};
use tokio::sync::mpsc::Sender;

/// Get URL from a message
/// Returns the first URL found in the message
/// If the message starts with "/url", it will return the URL from the reply message
/// If the message starts with "/url <URL>", it will return the URL
/// If no URL is found, it will return None
///
/// # Arguments
/// * `msg` - Message
/// # Returns
/// * `Option<String>` containing the URL
/// * `None` if no URL is found
/// # Example
fn get_url_from_message(msg: &Message) -> Option<String> {
    fn extract_first_link(text: &str) -> Option<String> {
        let link_regex = Regex::new(r"https?://\S+").unwrap();

        if let Some(mat) = link_regex.find(text) {
            Some(mat.as_str().to_string())
        } else {
            None
        }
    }

    if let Some(text) = msg.text() {
        if text.starts_with("/url") {
            if text.len() < 6 {
                if let Some(reply) = msg.reply_to_message() {
                    if let Some(reply_text) = reply.text() {
                        return extract_first_link(reply_text);
                    }
                }
            } else {
                let url_text = &text[5..];

                return extract_first_link(url_text);
            }
        }
    }

    None
}

pub async fn process_message(
    bot: Arc<teloxide::Bot>,
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
            if let Some(url) = get_url_from_message(&msg_copy) {
                Some((None, None, Some(url)))
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
    bot: Arc<teloxide::Bot>,
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