use std::error::Error;

use log::{error, info};
use structopt::StructOpt;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(subcommand)]
    command: Command,

    /// Path to the FIFO (default: /tmp/file2link.pipe)
    #[structopt(long, default_value = "/tmp/file2link.pipe")]
    path: String,
}

#[derive(StructOpt)]
enum Command {
    UpdatePermissions,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = Cli::from_args();

    match args.command {
        Command::UpdatePermissions => {
            let path = args.path;

            let mut file = OpenOptions::new().write(true).open(&path).await
                .map_err(|e| {
                    error!("Failed to open FIFO at {}: {}", path, e);

                    Box::new(e) as Box<dyn Error>
                })?;

            file.write_all(b"update_permissions\n").await
                .map_err(|e| {
                    error!("Failed to write to the FIFO: {}", e);
                    Box::new(e) as Box<dyn Error>
                })?;

            file.flush().await
                .map_err(|e| {
                    error!("Failed to flush the FIFO: {}", e);
                    Box::new(e) as Box<dyn Error>
                })?;

            info!("Command 'update_permissions' successfully sent to {}", path);
        }
    }

    Ok(())
}
