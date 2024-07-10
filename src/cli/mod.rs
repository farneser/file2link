use std::sync::Arc;

use log::{error, info, warn};
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;

use crate::chat_config;
use crate::chat_config::PermissionsConfig;
use crate::cli::cli::create_fifo;
use crate::config::Config;

pub mod cli;

pub async fn handle_cli(permissions: Arc<Mutex<PermissionsConfig>>) {
    let path = Config::instance().await.pipe_path();

    match create_fifo(&path).await {
        Ok(_) => info!("FIFO created at {}", path),
        Err(e) => {
            error!("Failed to create FIFO at {}: {}", path, e);

            return;
        }
    }

    let file = match OpenOptions::new().read(true).open(path.clone()).await {
        Ok(file) => file,
        Err(e) => {
            error!("Failed to open FIFO at {}: {}", path, e);

            return;
        }
    };

    let mut reader = BufReader::new(file).lines();

    loop {
        while let Some(line) = match reader.next_line().await {
            Ok(line) => line,
            Err(e) => {
                error!("Failed to read FIFO at {}: {}", path, e);

                return;
            }
        } {
            if line.trim() == "update_permissions" {
                let new_permissions = match chat_config::load_config().await {
                    Ok(new_permissions) => new_permissions,
                    Err(e) => {
                        warn!("Failed to load new permissions config, using old one. Error: {:?}", e);

                        continue;
                    }
                };

                let mut permissions = permissions.lock().await;

                *permissions = new_permissions;

                info!("Permissions updated successfully");
            } else if line.trim() == "shutdown" {
                info!("Shutting down command handled");

                return;
            }
        }
    }
}
