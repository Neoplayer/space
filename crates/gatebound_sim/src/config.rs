use gatebound_domain::{ConfigError, RuntimeConfig};
use serde::de::DeserializeOwned;
use std::fs;
use std::path::Path;

pub fn load_runtime_config(dir: &Path) -> Result<RuntimeConfig, ConfigError> {
    let config = RuntimeConfig {
        time: load_section(dir, "time_units.toml")?,
        galaxy: load_section(dir, "galaxy.toml")?,
        market: load_section(dir, "market.toml")?,
        pressure: load_section(dir, "economy_pressure.toml")?,
    };

    validate_runtime_config(&config)?;
    Ok(config)
}

pub fn validate_runtime_config(config: &RuntimeConfig) -> Result<(), ConfigError> {
    config.validate()
}

fn load_section<T>(dir: &Path, file_name: &str) -> Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    let path = dir.join(file_name);
    let raw = fs::read_to_string(&path)
        .map_err(|error| ConfigError::Io(format!("failed to read {file_name}: {error}")))?;
    toml_edit::de::from_str(&raw)
        .map_err(|error| ConfigError::Parse(format!("failed to parse {file_name}: {error}")))
}
