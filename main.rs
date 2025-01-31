mod service;

use anyhow::{bail, Context, Result};
use deadpool_postgres::Pool;
use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Response};
use hyper_util::rt::TokioIo;
use log::{error, info, warn, LevelFilter};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use serde_json::{json, to_string};
use std::{convert::Infallible, env::var, net::SocketAddr, process::exit};
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
	let port = var("PORT")
		.map(|value| value.parse::<u16>().expect("failed to parse PORT into a u16"))
		.unwrap_or(8000);
	let addr = SocketAddr::from(([127, 0, 0, 1], port));

	let db_url = match var("DB_URL") {
		Ok(url) => url,
		Err(_) => bail!("expected to find a DB_URL env var"),
	};

	let pool = get_static_pool(&db_url).await?;

	let listener = TcpListener::bind(addr).await?;
	info!("listening at http://127.0.0.1:{port}");

	loop {
		let (stream, _) = listener.accept().await?;
		let io = TokioIo::new(stream);

		spawn(async move {
			if let Err(err) = http1::Builder::new()
				.serve_connection(
					io,
					service_fn(move |request| async {
						let db_client = match pool.get().await {
							Ok(client) => client,
							Err(err) => {
								error!("failed to get client from pool: {err:?}");

								let response = Response::builder()
									.status(500)
									.header("content-type", "application/json")
									.body(Full::new(Bytes::from(
										"{\"error\":\"Whoops! We've hit a bump! Please try again later.\"}",
									)))
									.unwrap();

								return Ok::<_, Infallible>(response);
							}
						};

						let response = match service::service(db_client, request).await {
							Ok(response) => response,
							Err(err) => {
								warn!("service has thrown an error, giving 400: {err:?}");

								let text = to_string(&json!({
									"error": err.to_string()
								}))
								// we will always be able to serialize this
								.unwrap();

								let response = Response::builder()
									.status(500)
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
				error!("failed to server connection: {err:?}");
			}
		});
	}
}

async fn get_static_pool(db_url: &str) -> Result<&'static Pool> {
	let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
	builder.set_verify(SslVerifyMode::NONE);

	let connector = MakeTlsConnector::new(builder.build());
	let db_manager = deadpool_postgres::Manager::new(db_url.parse().context("failed to parse db url")?, connector);

	Ok(Box::leak(Box::new(Pool::builder(db_manager).max_size(16).build().unwrap())))
}
