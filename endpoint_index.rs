use anyhow::{anyhow, Result};
use dashmap::DashMap;
use deadpool_postgres::{Client, GenericClient};
use serde_json::{json, Map, Value};
use std::sync::{Arc, Mutex};

use crate::{config::Config, endpoint::Endpoint};

#[derive(Debug, Clone)]
struct Info {
	title: String,
	description: String,
	version: String,
}

pub struct EndpointIndex {
	info: Mutex<Info>,
	map: DashMap<String, Arc<Endpoint>>,
}

impl EndpointIndex {
	pub async fn fetch(db_client: &Client, config: &Config) -> Result<EndpointIndex> {
		let info = fetch_info(db_client, &config.info_query).await?;
		let endpoints = fetch_endpoints(db_client, &config.endpoints_query).await?;

		Ok(EndpointIndex {
			info: Mutex::new(info),
			map: DashMap::from_iter(endpoints),
		})
	}

	pub async fn refresh(&self, db_client: &Client, config: &Config) -> Result<()> {
		*self.info.lock().unwrap() = fetch_info(db_client, &config.info_query).await?;

		let endpoints = fetch_endpoints(db_client, &config.endpoints_query).await?;
		self.map.retain(|_, _| false);

		for (path, endpoint) in endpoints {
			self.map.insert(path, endpoint);
		}

		Ok(())
	}

	pub fn get_count(&self) -> usize {
		self.map.len()
	}

	pub fn get_schema(&self) -> Value {
		let mut endpoints = Map::new();

		// drop as soon as we're done
		{
			for item in self.map.iter() {
				endpoints.insert(item.key().clone(), item.value().get_schema());
			}
		}

		// we want to drop the mutex as early as possible
		let info = { self.info.lock().unwrap().clone() };

		json!({
			"openapi": "3.1.0",
			"info": {
				"title": info.title,
				"description": info.description,
				"version": info.version,
			},
			"paths": endpoints,
		})
	}

	pub fn get_endpoint(&self, path: &str) -> Result<Arc<Endpoint>> {
		let endpoint = self.map.get(path).ok_or(anyhow!("endpoint does not exist"))?;

		Ok(endpoint.clone())
	}
}

async fn fetch_endpoints(db_client: &Client, query: &str) -> Result<Vec<(String, Arc<Endpoint>)>> {
	db_client
		.query(query, &[])
		.await?
		.drain(..)
		.map(|row| {
			let mut path = None::<String>;
			let mut fn_name = None;
			let mut request = None;
			let mut response = None;

			for (index, col) in row.columns().iter().enumerate() {
				match col.name() {
					"path" => path = Some(row.try_get(index)?),
					"fn_name" => fn_name = Some(row.try_get(index)?),
					"request" => request = Some(row.try_get(index)?),
					"response" => response = Some(row.try_get(index)?),
					_ => (),
				}
			}

			let path = path.ok_or(anyhow!("path column was not returned from endpoints query"))?;
			let path = if path.starts_with("/") { path[1..].to_string() } else { path };

			let endpoint = Endpoint::from_config(
				fn_name.ok_or(anyhow!("fn_name column was not returned from endpoints query"))?,
				request.ok_or(anyhow!("request column was not returned from endpoints query"))?,
				response.ok_or(anyhow!("response column was not returned from endpoints query"))?,
			)?;

			Ok((path, Arc::new(endpoint)))
		})
		.collect()
}

async fn fetch_info(db_client: &Client, query: &str) -> Result<Info> {
	let row = db_client.query_one(query, &[]).await?;
	let mut title = None;
	let mut description = None;
	let mut version = None;

	for (index, col) in row.columns().iter().enumerate() {
		match col.name() {
			"title" => title = Some(row.try_get(index)?),
			"description" => description = Some(row.try_get(index)?),
			"version" => version = Some(row.try_get(index)?),
			_ => (),
		}
	}

	Ok(Info {
		title: title.ok_or(anyhow!("title column was not specified in info query"))?,
		description: description.ok_or(anyhow!("description column was not specified in info query"))?,
		version: version.ok_or(anyhow!("version column was not specified in info query"))?,
	})
}
