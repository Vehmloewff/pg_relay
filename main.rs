mod config;
mod endpoint;
mod endpoint_index;
mod service;

use anyhow::{Context, Result};
use config::Config;
use deadpool_postgres::Pool;
use endpoint_index::EndpointIndex;
use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Response};
use hyper_util::rt::TokioIo;
use log::{error, info, warn, LevelFilter};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use serde_json::{json, to_string};
use service::ServiceParams;
use std::{convert::Infallible, net::SocketAddr, process::exit};
use tokio::{net::TcpListener, spawn};

#[tokio::main]
async fn main() {
	env_logger::builder().filter(None, LevelFilter::Trace).init();

	if let Err(err) = main_impl().await {
		error!("failed to start server: {err:?}");
		exit(1);
	}
}

async fn main_impl() -> Result<()> {
	// We leak these variables so that we don't have to reference count them.
	// They will be needed for the rest of the program
	let config = leak(Config::load().await?);
	let pool = leak(get_db_pool(&config.db_url).await?);
	let methods_index = leak(EndpointIndex::fetch(&pool.get().await?, &config).await?);

	let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
	let listener = TcpListener::bind(addr).await?;
	info!("listening at http://127.0.0.1:{}", config.port);

	loop {
		let (stream, _) = listener.accept().await?;
		let io = TokioIo::new(stream);

		spawn(async move {
			if let Err(err) = http1::Builder::new()
				.serve_connection(
					io,
					service_fn(move |request| async {
						let params = ServiceParams {
							pool,
							methods_index,
							config,
							request,
						};

						let response = match service::service(params).await {
							Ok(response) => response,
							Err(err) => {
								warn!("service has thrown an error, giving 400: {err:?}");

								// we will always be able to serialize this, hence the unwrap
								let text = to_string(&json!({
									"error": err.to_string()
								}))
								.unwrap();

								// we have configured the builder correctly, we we won't panic
								let response = Response::builder()
									.status(400)
									.header("content-type", "application/json")
									.body(Full::new(Bytes::from(text)))
									.unwrap();

								return Ok::<_, Infallible>(response);
							}
						};

						Ok::<_, Infallible>(response)
					}),
				)
				.await
			{
				error!("failed to serve connection: {err:?}");
			}
		});
	}
}

async fn get_db_pool(db_url: &str) -> Result<Pool> {
	let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
	builder.set_verify(SslVerifyMode::NONE);

	let connector = MakeTlsConnector::new(builder.build());
	let db_manager = deadpool_postgres::Manager::new(db_url.parse().context("failed to parse db url")?, connector);

	Ok(Pool::builder(db_manager).max_size(16).build().unwrap())
}

fn leak<T>(value: T) -> &'static T {
	Box::leak(Box::new(value))
}
