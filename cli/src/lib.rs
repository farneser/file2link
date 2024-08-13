use crate::utils::send_command;
use log::{error, info};
use structopt::StructOpt;

pub mod utils;

#[derive(StructOpt)]
#[structopt(
    name = "File2Link CLI",
    about = "CLI tool for file2link",
)]
pub struct Cli {
    #[structopt(subcommand)]
    pub command: Command,

    /// Path to the FIFO (default: /tmp/file2link.pipe, env: F2L_PIPE_PATH)
    #[structopt(
        long,
        default_value = "/tmp/file2link.pipe",
        env = "F2L_PIPE_PATH",
        help = "Path to the FIFO"
    )]
    pub path: String,
}

#[derive(StructOpt)]
pub enum Command {
    #[structopt(about = "Updates the permissions from the config file")]
    UpdatePermissions,
    #[structopt(about = "Shutting down the system")]
    Shutdown,
}

pub struct CommandProcessor {
    path: String,
}

impl CommandProcessor {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    pub async fn process_command(&self, command: Command) {
        match command {
            Command::UpdatePermissions => {
                match send_command(&self.path, "update_permissions").await {
                    Ok(_) => info!("Command 'update_permissions' sent to {}", self.path),
                    Err(_) => error!("Failed to send command 'update_permissions' to {}", self.path),
                }
            }
            Command::Shutdown => {
                match send_command(&self.path, "shutdown").await {
                    Ok(_) => info!("Command 'shutdown' sent to {}", self.path),
                    Err(_) => error!("Failed to send command 'shutdown' to {}", self.path),
                }
            }
        }
    }
}