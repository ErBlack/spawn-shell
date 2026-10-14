use std::path::PathBuf;

use anyhow::Result;
pub use protocol::data_dir;

const PREVIOUS_IDENTIFIER: &str = "com.erblack.summon-terminal";
pub const LOCK_FILE: &str = "daemon.lock";

pub fn previous_data_dir() -> Result<PathBuf> {
    data_dir().map(|dir| dir.with_file_name(PREVIOUS_IDENTIFIER))
}

pub fn history_dir() -> Result<PathBuf> {
    data_dir().map(|dir| dir.join("history"))
}

pub fn session_file() -> Result<PathBuf> {
    data_dir().map(|dir| dir.join("session.json"))
}

pub fn lock_file() -> Result<PathBuf> {
    data_dir().map(|dir| dir.join(LOCK_FILE))
}
