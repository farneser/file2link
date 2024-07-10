use std::error::Error;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;

use log::{error, info};
use structopt::StructOpt;
use tokio::fs;
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

pub async fn create_fifo(path: &str) -> Result<(), String> {
    if !Path::new(&path).exists() {
        let c_path = std::ffi::CString::new(path).unwrap();
        let result = unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) };

        if result != 0 {
            error!("Failed to create FIFO at {}", path);

            return Err(format!("Failed to create FIFO at {}", path));
        }
    } else {
        let metadata = match fs::metadata(path).await {
            Ok(metadata) => metadata,
            Err(e) => {
                error!("Failed to get metadata for FIFO: {:?}", e);

                return Err(format!("Failed to get metadata for FIFO: {:?}", e));
            }
        };

        if !metadata.file_type().is_fifo() {
            error!("Path is not a FIFO: {:?}", path);

            return Err(format!("Path is not a FIFO: {:?}", path));
        }
    }

    Ok(())
}
#[cfg(test)]
mod test {
    use std::env;
    use std::fs;

    use assert_cmd::Command;
    use nanoid::nanoid;

    use super::*;

    async fn create_rnd_file() -> String {
        let path = format!("/tmp/f2l-test-temp-{}.pipe", nanoid!());

        fs::write(&path, "").unwrap();

        path
    }

    async fn delete_file(path: &str) {
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_send_command_success() {
        let binding = create_rnd_file().await;
        let path = binding.as_str();

        let result = send_command(path, "test_command").await;

        assert!(result.is_ok());

        let content = fs::read_to_string(path).unwrap();
        assert_eq!(content, "test_command\n");

        delete_file(path).await;
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
        let binding = create_rnd_file().await;
        let path = binding.as_str();

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();

        cmd.arg("--path").arg(path).arg("update-permissions");

        cmd.assert().success();

        let content = fs::read_to_string(path).unwrap();
        println!("content: {content}");

        assert_eq!(content, "update_permissions\n");

        delete_file(path).await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_shutdown() {
        let binding = create_rnd_file().await;
        let path = binding.as_str();

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();
        cmd.arg("--path").arg(path).arg("shutdown");

        cmd.assert().success();

        let content = fs::read_to_string(path).unwrap();

        assert_eq!(content, "shutdown\n");

        delete_file(path).await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_cli_default_path() {
        let binding = create_rnd_file().await;
        let path = binding.as_str();

        env::set_var("F2L_PIPE_PATH", path);

        let mut cmd = Command::cargo_bin("f2l-cli").unwrap();

        cmd.arg("shutdown");
        cmd.assert().success();

        let content = match fs::read_to_string(path) {
            Ok(content) => { content }
            Err(_) => "".to_owned()
        };

        assert_eq!(content, "shutdown\n");

        delete_file(path).await;
    }
}
