use anyhow::Result;
use deadpool_postgres::Client;
use http_body_util::Full;
use hyper::{
	body::{Bytes, Incoming},
	Request,
};

pub async fn service(db_client: Client, request: Request<Incoming>) -> Result<Response<Full<Bytes>>> {}
