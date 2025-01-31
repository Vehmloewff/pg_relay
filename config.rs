use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use tokio::fs::read_to_string;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
	pub port: u16,
	pub db_url: String,
	pub info_query: String,
	pub methods_query: String,
}

impl Config {
	pub async fn load() -> Result<Config> {
		let text = read_to_string("pg_relay.toml").await?;

		Ok(from_str(&text)?)
	}
}
