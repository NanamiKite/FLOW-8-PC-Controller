//! Persistent application-level FLOW 8 client identity.
//!
//! The device sends its own ID in 0x35. Our independent client UUID is stored
//! before the first 0x39, then reused across connections and application runs.

use std::{
    env, fs,
    fs::OpenOptions,
    io::{self, Write},
    path::{Path, PathBuf},
};

use tracing::{debug, info};
use uuid::Uuid;

const ID_FILE: &str = "client-id.txt";

fn config_directory() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let root = env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .ok_or("LOCALAPPDATA is unavailable; cannot persist FLOW 8 client identity")?;
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return Err("LOCALAPPDATA is not an absolute path".into());
        }
        Ok(root.join("FLOW 8 PC Controller"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let root = env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                env::var_os("HOME")
                    .filter(|value| !value.is_empty())
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .ok_or("no per-user configuration directory is available")?;
        if !root.is_absolute() {
            return Err("per-user configuration directory is not an absolute path".into());
        }
        Ok(root.join("flow8-pc-controller"))
    }
}

fn read_existing(path: &Path) -> Result<[u8; 16], String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "cannot inspect FLOW 8 client identity {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "FLOW 8 client identity path is not a regular file: {}",
            path.display()
        ));
    }
    let text = fs::read_to_string(path).map_err(|error| {
        format!(
            "cannot read FLOW 8 client identity {}: {error}",
            path.display()
        )
    })?;
    let uuid = Uuid::parse_str(text.trim()).map_err(|_| {
        format!(
            "FLOW 8 client identity file is invalid; inspect or restore {}",
            path.display()
        )
    })?;
    if uuid.is_nil() {
        return Err(format!("FLOW 8 client identity is nil: {}", path.display()));
    }
    debug!(path = %path.display(), "loaded persistent FLOW 8 client identity");
    Ok(*uuid.as_bytes())
}

pub(super) fn load_or_create() -> Result<[u8; 16], String> {
    let directory = config_directory()?;
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "cannot create FLOW 8 configuration directory {}: {error}",
            directory.display()
        )
    })?;
    let metadata = fs::symlink_metadata(&directory).map_err(|error| error.to_string())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "FLOW 8 configuration path is not a real directory: {}",
            directory.display()
        ));
    }
    let path = directory.join(ID_FILE);
    match read_existing(&path) {
        Ok(id) => return Ok(id),
        Err(_) if !path.exists() => {}
        Err(error) => return Err(error),
    }

    let uuid = Uuid::new_v4();
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return read_existing(&path);
        }
        Err(error) => {
            return Err(format!(
                "cannot persist FLOW 8 client identity {}: {error}",
                path.display()
            ));
        }
    };
    writeln!(file, "{uuid}")
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            format!(
                "cannot finish persisting FLOW 8 client identity {}: {error}",
                path.display()
            )
        })?;
    info!(path = %path.display(), "created persistent FLOW 8 client identity");
    Ok(*uuid.as_bytes())
}
