use std::error::Error;

use log::{error, info};
use structopt::StructOpt;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

#[derive(StructOpt)]
#[structopt(
    name = "File2Link CLI",
    about = "CLI tool for file2link",
)]
struct Cli {
    #[structopt(subcommand)]
    command: Command,

    /// Path to the FIFO (default: /tmp/file2link.pipe, env: F2L_PIPE_PATH)
    #[structopt(
        long,
        default_value = "/tmp/file2link.pipe",
        env = "F2L_PIPE_PATH",
        help = "Path to the FIFO"
    )]
    path: String,
}

#[derive(StructOpt)]
enum Command {
    #[structopt(about = "Updates the permissions from the config file")]
    UpdatePermissions,
    #[structopt(about = "Shutting down the system")]
    Shutdown,
}
#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = Cli::from_args();
    let path = args.path;

    match args.command {
        Command::UpdatePermissions => {
            match send_command(&path, "update_permissions").await {
                Ok(_) => info!("Command 'update_permissions' sent to {}", path),
                Err(_) => error!("Failed to send command 'update_permissions' to {}", path),
            };
        }
        Command::Shutdown => {
            match send_command(&path, "shutdown").await {
                Ok(_) => info!("Command 'shutdown' sent to {}", path),
                Err(_) => error!("Failed to send command 'shutdown' to {}", path),
            };
        }
    }

    Ok(())
}

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

#[cfg(test)]
mod test {
    use std::env;
    use std::fs;

    use assert_cmd::Command;
    use tempfile::NamedTempFile;

    use super::*;

    #[tokio::test]
    #[serial_test::serial]
    async fn test_send_command_success() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let result = send_command(path, "test_command").await;

        assert!(result.is_ok());

        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, "test_command\n");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_send_command_failure() {
        let path = "/invalid/path/to/file.pipe";

        let result = send_command(path, "test_command").await;

        assert!(result.is_err());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_update_permissions() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();

        cmd.arg("--path").arg(path).arg("update-permissions");

        cmd.assert().success();

        let content = fs::read_to_string(path).unwrap();
        println!("content: {content}");

        assert_eq!(content, "update_permissions\n");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_shutdown() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_str().unwrap();

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();
        cmd.arg("--path").arg(path).arg("shutdown");

        cmd.assert().success();

        let content = fs::read_to_string(path).unwrap();

        assert_eq!(content, "shutdown\n");
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_default_path() {
        let temp_file = NamedTempFile::new().unwrap();

        env::set_var("F2L_PIPE_PATH", temp_file.path().to_str().unwrap());

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();

        cmd.arg("shutdown");
        cmd.assert().success();

        let content = match fs::read_to_string(temp_file.path()) {
            Ok(content) => { content }
            Err(_) => "".to_owned()
        };

        assert_eq!(content, "shutdown\n");
    }
}
