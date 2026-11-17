//! Cross-platform data paths.
//!
//! - Windows: `%APPDATA%\modelfolio`
//! - macOS:   `~/Library/Application Support/modelfolio`
//! - Linux:   `$XDG_DATA_HOME/modelfolio` (default `~/.local/share/modelfolio`)

use std::io;
use std::path::PathBuf;

pub fn data_dir() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });
    base.join("modelfolio")
}

pub fn ensure_dirs() -> io::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::create_dir_all(exports_dir())?;
    Ok(())
}

pub fn config_path() -> PathBuf {
    data_dir().join("config.json")
}

pub fn cache_path() -> PathBuf {
    data_dir().join("cache.sqlite")
}

pub fn exports_dir() -> PathBuf {
    data_dir().join("exports")
}
