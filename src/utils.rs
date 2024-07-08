use std::io;
use std::path::Path;

use tokio::fs;

pub fn get_file_name_from_path(path: &str) -> Option<&str> {
    Path::new(path).file_name()?.to_str()
}

pub async fn get_file_size(path: &str) -> io::Result<u64> {
    let metadata = fs::metadata(path).await.expect("Failed to read file metadata");

    Ok(metadata.len())
}

pub async fn create_directory(dir_name: &str) -> io::Result<()> {
    fs::create_dir_all(dir_name).await?;

    Ok(())
}

pub fn get_folder_and_file_name(path: &str) -> Option<String> {
    let path = Path::new(path);

    let parent_dir = path.parent()?.file_name()?.to_string_lossy().into_owned();

    let file_name = path.file_name()?.to_string_lossy().into_owned();

    Some(format!("{}/{}", parent_dir, file_name))
}
