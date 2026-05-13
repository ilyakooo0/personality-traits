use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub events_file: PathBuf,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: std::env::var("DATABASE_URL")
                .context("DATABASE_URL must be set (see .env.example)")?,
            events_file: PathBuf::from(
                std::env::var("EVENTS_FILE").context("EVENTS_FILE must be set")?,
            ),
            data_dir: PathBuf::from(
                std::env::var("DATA_DIR").context("DATA_DIR must be set")?,
            ),
        })
    }
}
