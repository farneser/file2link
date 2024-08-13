use std::error::Error;
use std::os::unix::fs::FileTypeExt;

use crate::utils::send_command;
use cli::Cli;
use structopt::StructOpt;
use tokio::io::AsyncWriteExt;

pub mod utils;

#[allow(dead_code)]
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();

    let args = Cli::from_args();
    let path = args.path;

    cli::CommandProcessor::new(path).process_command(args.command).await;

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
