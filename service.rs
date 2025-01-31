use anyhow::{bail, Result};
use deadpool_postgres::Client;
use http_body_util::Full;
use hyper::{
	body::{Bytes, Incoming},
	Method, Request, Response,
};

pub async fn service(db_client: Client, request: Request<Incoming>) -> Result<Response<Full<Bytes>>> {
	match (request.method(), request.uri().path()) {
		(Method::GET, "/") => {}
		(_, _) => bail!("endpoint does not exist"),
	}
}
