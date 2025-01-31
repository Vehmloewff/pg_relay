use anyhow::{anyhow, bail, Context, Result};
use deadpool_postgres::Pool;
use http_body_util::{BodyExt, Full};
use hyper::{
	body::{Bytes, Incoming},
	Method, Request, Response,
};
use serde_json::{from_slice, json, to_string, Value};

use crate::{config::Config, methods_index::MethodsIndex};

pub struct ServiceParams<'a> {
	pub pool: &'a Pool,
	pub methods_index: &'a MethodsIndex,
	pub config: &'a Config,
	pub request: Request<Incoming>,
}

pub async fn service(params: ServiceParams<'_>) -> Result<Response<Full<Bytes>>> {
	let response_json = match (params.request.method(), params.request.uri().path()) {
		(&Method::GET, "/") => params.methods_index.get_schema(),
		(&Method::DELETE, "/methods") => {
			if let Some(requested_token) = params.config.admin_token.as_ref() {
				let token_raw = params
					.request
					.headers()
					.get("admin-token")
					.ok_or(anyhow!("expected an admin-token header"))?;

				let token = token_raw.to_str().context("admin-token is not a valid string")?;

				if token != requested_token.as_str() {
					bail!("incorrect admin-token");
				}
			}

			let old_method_count = params.methods_index.get_count();
			params.methods_index.refresh(&params.pool.get().await?, params.config).await?;
			let new_method_count = params.methods_index.get_count();

			json!({
				"old_method_count": old_method_count,
				"new_method_count": new_method_count,
			})
		}
		(&Method::POST, path) => {
			let method = params.methods_index.get_method(path)?;
			let db_pool = params.pool.get().await?;
			let whole_request = params.request.into_body().collect().await?.to_bytes();
			let request_data = from_slice::<Value>(&whole_request)?;

			method.run(&db_pool, &request_data).await?
		}
		(_, _) => bail!("endpoint does not exist"),
	};

	let response_bytes = Bytes::from(to_string(&response_json)?);

	Ok(Response::builder()
		.header("content-type", "application/json")
		.body(Full::new(response_bytes))
		.unwrap())
}
