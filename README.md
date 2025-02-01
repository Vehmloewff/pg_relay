# pg_relay

> Currently in Alpha, so missing features can be expected. Pull requests are welcome!

**pg_relay** is a lightweight API gateway that directly maps HTTP endpoints to PostgreSQL stored functions. This allows developers to focus solely on writing SQL, without the need for boilerplate API code. Define your database functions, configure pg_relay, and expose an instantly queryable API.

## Features

- **Provides API endpoints** that are backed by PostgreSQL stored functions
- **Validates requests** to ensure they conform to a json schema, stored in the database
- **Simple configuration** via a TOML file
- **Exposes an OpenAPI specification** for automatic documentation of your API

## Configuration

pg_relay is configured via a `pg_relay.toml` file. The following options are available:

```toml
port = 8080
db-url = "postgres://user:password@localhost/dbname"

# The query that will be used to retrieve API metadata from the database. This information is displayed in the open api schema. The order of the columns returned does not matter.
info-query = "SELECT title, description, version FROM api_info"

# The query that will be used to load the API endpoints. There is more information about this below. The order of the columns returned does not matter.
endpoints-query = "SELECT path, fn_name, request, response FROM api_endpoints"

# The optional (but recommended) token, required for flushing the cache. More on this later.
admin-token = "supersecret"
```

Currently, all database connections are made with TLS. Ideally, to use TLS or not should be a config option. Pull requests are welcome!

### Endpoints Options

Each column in the endpoints query is considered an "endpoint option". All options are required.

- `path` - the path that the method is to be exposed at. Can contain slashes.
- `fn_name` - the postgres function name to call when this endpoint is hit. One argument will be passed into this function, of type `jsonb`. The contents of this argument will be the request body, and it is guaranteed to match the request schema.
- `request` - the request schema. Must be json of some sort, and must be a valid json schema.
- `response` - the response schema. Currently this is not used for any purpose other than being displaying in the open api schema.

### Database Setup

To use pg_relay, your PostgreSQL database should have tables that store API metadata and method definitions. Here’s an example schema:

```sql
CREATE TABLE api_info (
    title TEXT NOT NULL,
    description TEXT NOT NULL,
    version TEXT NOT NULL
);

INSERT INTO api_info (title, description, version)
VALUES ('My API', 'API for my app', '1.0.0');

CREATE TABLE api_endpoints (
    path TEXT PRIMARY KEY,
    fn_name TEXT NOT NULL,
    request JSONB NOT NULL,
    response JSONB NOT NULL
);
```

### Simple Example

This example defines a simple function and exposes it as an API endpoint:

```sql
CREATE FUNCTION example_function(param TEXT) RETURNS JSONB AS $$
BEGIN
    RETURN jsonb_build_object('result', 'Hello, ' || param || '!');
END;
$$ LANGUAGE plpgsql;

INSERT INTO api_endpoints (path, fn_name, request, response)
VALUES (
  '/example_method',
  'example_function',
  '{
    "type": "object",
    "properties": {
      "param": {
        "type": "string"
      }
    }
  }',
  '{
    "type": "object",
    "properties": {
      "result": {
        "type": "string"
      }
    }
  }'
);
```

### More Complex Example

Here’s a setup for a user-related function:

```sql
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT UNIQUE NOT NULL
);

INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com');

CREATE FUNCTION get_user(email TEXT) RETURNS JSONB AS $$
DECLARE
    user_record RECORD;
BEGIN
    SELECT * INTO user_record FROM users WHERE users.email = get_user.email;
    IF NOT FOUND THEN
        RETURN jsonb_build_object('error', 'User not found');
    END IF;
    RETURN jsonb_build_object(
      'id', user_record.id,
      'name', user_record.name,
      'email', user_record.email
    );
END;
$$ LANGUAGE plpgsql;

INSERT INTO api_endpoints (path, fn_name, request, response)
VALUES (
  '/get_user',
  'get_user',
  '{
    "type": "object",
    "properties": {
      "email": { "type": "string" }
    }
  }',
  '{
    "type": "object",
    "properties": {
      "id": { "type": "integer" },
      "name": { "type": "string" },
      "email": { "type": "string" }
    }
  }'
);
```

## Usage

Once configured and running, pg_relay exposes http endpoints for your functions. Requests are validated against their schemas before execution.

Example:
```sh
curl -X POST http://localhost:8080/example_method -H "Content-Type: application/json" -d '{"param": "World"}'
```

Response:
```json
{"result": "Hello, World!"}
```

For the user-related function:
```sh
curl -X POST http://localhost:8080/get_user -H "Content-Type: application/json" -d '{"email": "alice@example.com"}'
```

Response:
```json
{"id": 1, "name": "Alice", "email": "alice@example.com"}
```

## OpenAPI Support

pg_relay supports generating an OpenAPI specification for your API automatically. This allows you to integrate with tools like Swagger UI or Redoc for interactive API documentation.

To retrieve the OpenAPI specification, send a GET request to `/`:

```sh
curl -X GET http://localhost:8080
```

This will return a JSON representation of the OpenAPI spec, including all available endpoints and their expected request/response formats.

## Error Handling

Currently, all error messages are piped directly to the client as a json `{ "error": "..." }` response, so there is a lot of room for improvement here. Pull requests are welcome!

## Flushing API endpoints

The list of api endpoints and meta information is loaded when the server starts up. However, after deploying changes to your database, you'll likely want to refresh this cache. This can be done by sending a `DELETE` request to `/cache`.

```sh
curl -X DELETE http://localhost:8080/cache -H "admin-token: supersecret"
```

This reloads API endpoints and metadata from the database. If `admin_token` is configured, the request must include a matching `admin-token` header.

## License

pg_relay is open-source and licensed under the MIT License.
