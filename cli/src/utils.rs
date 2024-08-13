use std::error::Error;

use log::{error};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

pub async fn send_command(path: &str, command: &str) -> Result<(), Box<dyn Error>> {
    let mut file = OpenOptions::new().write(true).open(&path).await
        .map_err(|e| {
            error!("Failed to open FIFO at {}: {}", path, e);

            Box::new(e) as Box<dyn Error>
        })?;

    file.write_all(format!("{}\n", command).as_bytes()).await
        .map_err(|e| {
            error!("Failed to write to the FIFO: {}", e);

            Box::new(e) as Box<dyn Error>
        })?;

    file.flush().await
        .map_err(|e| {
            error!("Failed to flush the FIFO: {}", e);

            Box::new(e) as Box<dyn Error>
        })?;

    Ok(())
}

