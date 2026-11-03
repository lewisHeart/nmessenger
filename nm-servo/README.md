# nm-servo

`nm-servo` is the Actix Web backend for NMessenger. It uses PostgreSQL for
persistence, Redis for presence, rate limits and event distribution, and
`nm-protocol` for shared HTTP and WebSocket types.

## Running locally

From the repository root, start the dependencies and export the configuration:

```sh
cp .env.example .env
make infra
set -a; . ./.env; set +a
make server
```

Alternatively, `make up` runs the backend and dependencies in Compose. The server
applies SQLx migrations during startup and exposes `/health` and `/v1/health`.

## Environment

| Variable | Description | Default |
| --- | --- | --- |
| `DATABASE_URL` | PostgreSQL connection URL | Required |
| `REDIS_URL` | Redis connection URL | Required |
| `BIND_ADDR` | HTTP listen address | `0.0.0.0:8080` |
| `UPLOAD_DIR` | Directory for uploaded files | `data/uploads` |
| `SESSION_TTL_SECS` | Session lifetime, 60–7776000 seconds | `2592000` |
| `USER_UPLOAD_QUOTA_BYTES` | Per-account quota, 25 MiB–1 TiB | `536870912` |
| `RUST_LOG` | Tracing filter | `nm_servo=info,actix_web=info` |

The first registered account becomes the initial administrator. This assignment
happens once, even if the account is later removed or its administrator role is
revoked. Existing installations retain their administrators and assign the role
to the earliest human account if that account is still active.

## API and behavior

HTTP routes are under `/v1`; authenticated requests use bearer tokens. Real-time
events use `/v1/events` and authenticate in the first WebSocket message. The
server persists messages and sessions in PostgreSQL and publishes events through
Redis. Clients must resynchronize after reconnecting. Uploaded files are stored
under `UPLOAD_DIR`; instances serving the same deployment need shared storage.

Bot owners can create and list accounts using `/v1/bots`, update their names,
and rotate tokens. Bots use the same encrypted message protocol as other
participants. The server does not include a built-in bot-management assistant.

## Tests

From the repository root:

```sh
make test-infra
NM_TEST_DATABASE_URL=postgres://nmessenger:nmessenger_test@127.0.0.1:55439/nmessenger_test \
NM_TEST_REDIS_URL=redis://127.0.0.1:56379/0 \
  cargo test -p nm-servo -- --test-threads=1
make test-stop
```

Integration tests require both environment variables and skip if neither is set.
Use dedicated test services, not a production database or Redis instance.
