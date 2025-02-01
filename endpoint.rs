use anyhow::{anyhow, Result};
use deadpool_postgres::{Client, GenericClient};
use jsonschema::{validator_for, Validator};
use serde_json::{json, Value};

pub struct Endpoint {
	request_validator: Validator,
	query: String,
	request_schema: Value,
	response_schema: Value,
}

impl Endpoint {
	pub fn from_config(fn_name: String, request: Value, response: Value) -> Result<Endpoint> {
		Ok(Endpoint {
			request_validator: validator_for(&request)?,
			request_schema: request,
			response_schema: response,
			query: format!("select {}($1)", fn_name),
		})
	}

	pub async fn run(&self, db_client: &Client, request_data: &Value) -> Result<Value> {
		self.request_validator
			.validate(request_data)
			.map_err(|err| anyhow!(err.to_string()))?;

		let statement = db_client.prepare_cached(&self.query).await?;
		let result = db_client.query_one(&statement, &[request_data]).await?.try_get::<_, Value>(0)?;

		Ok(result)
	}

	pub fn get_schema(&self) -> Value {
		json!({
			"post": {
				"summary": "summary not available",
				"requestBody": {
					"required": true,
					"content": {
						"application/json": {
							"schema": &self.request_schema
						}
					}
				},
				"responses": {
					"200": {
						"description": "A healthy response for this endpoint",
						"content": {
							"application/json": {
								"schema": &self.response_schema
							}
						}
					},
					"400": {
						"description": "An unhealthy response from this endpoint, indicating an error that can avoided",
						"content": {
							"application/json": {
								"schema": {
									"type": "object",
									"required": ["error"],
									"additionalProperties": false,
									"properties": {
										"error": {
											"type": "string"
										}
									}
								}
							}
						}
					},
					"500": {
						"description": "An unhealthy response from this endpoint, indicating some sort of internal failure",
						"content": {
							"application/json": {
								"schema": {
									"type": "object",
									"required": ["error"],
									"additionalProperties": false,
									"properties": {
										"error": {
											"type": "string"
										}
									}
								}
							}
						}
					}
				}
			}
		})
	}
}
