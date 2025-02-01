use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;
use toml::from_str;

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
	pub port: u16,
	#[serde(rename = "db-url")]
	pub db_url: String,

	#[serde(rename = "info-query")]
	pub info_query: String,

	#[serde(rename = "endpoints-query")]
	pub endpoints_query: String,

	#[serde(rename = "admin-key")]
	pub admin_key: Option<String>,
}

impl Config {
	pub async fn load() -> Result<Config> {
		let text = read_to_string("pg_relay.toml")
			.await
			.context("failed to read config from pg_relay.toml")?;

		Ok(from_str(&text).context("failed to parse config at pg_relay.toml")?)
	}
}
