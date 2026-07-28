use std::sync::OnceLock;

use crate::{log::LogConfig, shared::config::Config};

#[derive(clap::Parser)]
pub struct AppConfig {
    #[clap(long, env)]
    pub rpc: String,

    #[clap(long, env)]
    pub chain_id: String,

    #[clap(long, env, default_value_t = String::from("config.toml"), value_parser = clap::builder::ValueParser::new(file_exists))]
    pub config_path: String,

    #[clap(long, env)]
    pub slack: Option<String>,

    #[clap(long, env, default_value_t = 9184)]
    pub prometheus_port: u64,

    #[clap(long, env, default_value_t = u32::MAX)]
    pub initial_block_height: u32,

    #[clap(long, env, default_value_t = 3)]
    pub sleep_for: u64,

    #[clap(flatten)]
    pub log: LogConfig,

    /// Parsed `config_path`, read from disk once on first access.
    #[clap(skip)]
    config: OnceLock<Config>,
}

fn file_exists(path: &str) -> Result<String, String> {
    if std::path::Path::new(path).exists() {
        Ok(path.to_string())
    } else {
        Err(format!("Config file '{}' does not exist", path))
    }
}

impl AppConfig {
    /// The toml configuration. Parsed on first call and cached: this is read
    /// from every check and metric constructor, and re-reading the file each
    /// time also meant the process could pick up a half-written config.
    pub fn get_config(&self) -> &Config {
        self.config.get_or_init(|| {
            let raw = std::fs::read_to_string(&self.config_path).unwrap_or_else(|err| {
                panic!("Failed to read config file '{}': {}", self.config_path, err)
            });
            toml::de::from_str(&raw).unwrap_or_else(|err| {
                panic!(
                    "Failed to parse config file '{}': {}",
                    self.config_path, err
                )
            })
        })
    }
}
