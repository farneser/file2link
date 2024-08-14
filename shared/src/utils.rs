use log::error;
use std::io;
use std::os::unix::fs::FileTypeExt;
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

pub async fn create_fifo(path: &str) -> Result<(), String> {
    if !Path::new(&path).exists() {
        let c_path = std::ffi::CString::new(path).unwrap();
        let result = unsafe {
            libc::mkfifo(c_path.as_ptr(), 0o644)
        };

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