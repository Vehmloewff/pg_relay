use anyhow::{anyhow, Result};
use dashmap::DashMap;
use deadpool_postgres::{Client, GenericClient};
use serde_json::{json, Map, Value};
use std::sync::Mutex;

use crate::{config::Config, method::Method};

struct Info {
	title: String,
	description: String,
	version: String,
}

pub struct MethodsIndex {
	info: Mutex<Info>,
	map: DashMap<String, Method>,
}

impl MethodsIndex {
	pub async fn fetch(db_client: &Client, config: &Config) -> Result<MethodsIndex> {
		let info = fetch_info(db_client, &config.info_query).await?;
		let methods = fetch_methods(db_client, &config.methods_query).await?;

		Ok(MethodsIndex {
			info: Mutex::new(info),
			map: DashMap::from_iter(methods),
		})
	}

	pub async fn refresh(&self, db_client: &Client, config: &Config) -> Result<()> {
		*self.info.lock().unwrap() = fetch_info(db_client, &config.info_query).await?;

		let methods = fetch_methods(db_client, &config.methods_query).await?;
		self.map.retain(|_, _| false);

		for (path, method) in methods {
			self.map.insert(path, method);
		}

		Ok(())
	}

	pub fn get_schema(&self) -> Value {
		let mut endpoints = Map::new();

		for item in self.map.iter() {
			endpoints.insert(item.key().clone(), item.value().get_schema());
		}

		json!({
			"openapi": "3.1.0",
			"info": {
				"title": "no title",
				"description": "also, not description",
				"version": "0.1.0"
			},
			"paths": endpoints,
		})
	}
}

pub async fn fetch_methods(db_client: &Client, query: &str) -> Result<Vec<(String, Method)>> {
	db_client
		.query(query, &[])
		.await?
		.drain(..)
		.map(|row| {
			let mut path = None;
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

			let path = path.ok_or(anyhow!("path column was not returned from methods query"))?;

			let method = Method::from_config(
				fn_name.ok_or(anyhow!("fn_name column was not returned from methods query"))?,
				request.ok_or(anyhow!("request column was not returned from methods query"))?,
				response.ok_or(anyhow!("response column was not returned from methdos query"))?,
			)?;

			Ok((path, method))
		})
		.collect()
}

pub async fn fetch_info(db_client: &Client, query: &str) -> Result<Info> {
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
