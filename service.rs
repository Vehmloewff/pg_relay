use anyhow::{anyhow, bail, Context, Result};
use deadpool_postgres::Pool;
use http_body_util::{BodyExt, Full};
use hyper::{
	body::{Bytes, Incoming},
	Method, Request, Response,
};
use serde_json::{from_slice, json, to_string, Value};

use crate::{config::Config, endpoint_index::EndpointIndex};

pub struct ServiceParams<'a> {
	pub pool: &'a Pool,
	pub endpoint_index: &'a EndpointIndex,
	pub config: &'a Config,
	pub request: Request<Incoming>,
}

pub async fn service(params: ServiceParams<'_>) -> Result<Response<Full<Bytes>>> {
	let response_json = match (params.request.method(), params.request.uri().path()) {
		(&Method::GET, "/") => params.endpoint_index.get_schema(),
		(&Method::DELETE, "/cache") => {
			if let Some(requested_token) = params.config.admin_key.as_ref() {
				let token_raw = params
					.request
					.headers()
					.get("admin-key")
					.ok_or(anyhow!("expected an admin-token header"))?;

				let token = token_raw.to_str().context("admin-token is not a valid string")?;

				if token != requested_token.as_str() {
					bail!("incorrect admin-token");
				}
			}

			let old_endpoint_count = params.endpoint_index.get_count();
			params.endpoint_index.refresh(&params.pool.get().await?, params.config).await?;
			let new_endpoint_count = params.endpoint_index.get_count();

			json!({
				"old_endpoint_count": old_endpoint_count,
				"new_endpoint_count": new_endpoint_count,
			})
		}
		(&Method::POST, path) => {
			let endpoint = params.endpoint_index.get_endpoint(path)?;
			let db_pool = params.pool.get().await?;
			let whole_request = params.request.into_body().collect().await?.to_bytes();
			let request_data = from_slice::<Value>(&whole_request)?;

			endpoint.run(&db_pool, &request_data).await?
		}
		(_, _) => bail!("endpoint does not exist"),
	};

	let response_bytes = Bytes::from(to_string(&response_json)?);

	Ok(Response::builder()
		.header("content-type", "application/json")
		.body(Full::new(response_bytes))
		.unwrap())
}
