use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;
use toml::from_str;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
	pub port: u16,
	pub db_url: String,
	pub info_query: String,
	pub methods_query: String,
	pub admin_token: Option<String>,
}

impl Config {
	pub async fn load() -> Result<Config> {
		let text = read_to_string("pg_relay.toml")
			.await
			.context("failed to read config from pg_relay.toml")?;

		Ok(from_str(&text).context("failed to parse config at pg_relay.toml")?)
	}
}
