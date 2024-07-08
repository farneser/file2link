use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use log::{error, info, warn};
use tokio::fs;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::chat_config;
use crate::chat_config::PermissionsConfig;
use crate::config::Config;

pub async fn handle_cli(
    permissions: Arc<Mutex<PermissionsConfig>>,
    shutdown_token: CancellationToken,
) {
    let path = Config::instance().await.pipe_path();

    info!("Shutting token is cancelled: {}", shutdown_token.is_cancelled());

    if !Path::new(&path).exists() {
        let c_path = std::ffi::CString::new(path.clone()).unwrap();
        let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };

        if result != 0 {
            error!("Failed to create FIFO at {}", path);

            return;
        }
    } else {
        let metadata = match fs::metadata(path.clone()).await {
            Ok(metadata) => metadata,
            Err(e) => {
                error!("Failed to get metadata for FIFO: {:?}", e);

                return;
            }
        };

        if !metadata.file_type().is_fifo() {
            error!("Path is not a FIFO: {:?}", path);

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
        if shutdown_token.is_cancelled() {
            info!("Shutting down CLI handler");
            break;
        }

        match timeout(Duration::from_secs(1), reader.next_line()).await {
            Ok(Ok(Some(line))) => {
                info!("Received command: {}", line.trim());

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
                }
            }
            Ok(Ok(None)) => {
                warn!("FIFO closed");
                break;
            }
            Ok(Err(e)) => {
                error!("Error reading from FIFO: {:?}", e);
                return;
            }
            Err(_) => {
                if shutdown_token.is_cancelled() {
                    info!("Shutting down CLI handler");
                    break;
                }
            }
        }

        sleep(Duration::from_millis(100)).await;
    }
}