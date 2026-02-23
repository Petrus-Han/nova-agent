use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub llm: Option<LlmConfig>,
    pub performance: Option<PerformanceConfig>,
}

#[derive(Debug, Deserialize)]
pub struct LlmConfig {
    pub provider: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PerformanceConfig {
    pub threads: Option<usize>,
    pub cache_size: Option<String>,
}

/// Load config from ~/.nova/config.toml if it exists.
pub fn load_config() -> anyhow::Result<Config> {
    let config_path = config_path();

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".nova")
        .join("config.toml")
}
