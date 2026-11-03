# Operations & Deployment

This guide documents configuring, hosting, and maintaining production deployments of `nm-servo`.

---

## 1. Environment Configuration

Copy `.env.example` to `.env` and configure settings for your production domain and credentials:

| Variable | Description | Default / Example |
| :--- | :--- | :--- |
| `DATABASE_URL` | PostgreSQL connection string | `postgres://nmessenger:password@postgres:5432/nmessenger` |
| `REDIS_URL` | Redis connection string | `redis://:password@redis:6379/0` |
| `BIND_ADDR` | HTTP listen address inside the container | `0.0.0.0:8080` |
| `UPLOAD_DIR` | Filesystem path for uploaded attachments | `/app/data/uploads` |
| `SESSION_TTL_SECS` | Session token lifetime (60 to 7776000 s) | `2592000` (30 days) |
| `USER_UPLOAD_QUOTA_BYTES` | Maximum total upload quota per account | `536870912` (512 MiB) |
| `RUST_LOG` | Tracing logging level | `nm_servo=info,actix_web=info` |
| `GITHUB_WEBHOOK_SECRET` | HMAC secret for verifying release webhooks | Random 32+ character string |
| `NM_DOMAIN` | Production domain for Caddy TLS | `messenger.example.com` |

---

## 2. Production Deployment with HTTPS

Production deployments use **Caddy** to automatically provision and renew Let's Encrypt TLS certificates:

```sh
# 1. Edit .env with secure passwords and your domain name:
# POSTGRES_PASSWORD=generate-a-strong-password
# REDIS_PASSWORD=generate-a-strong-password
# NM_DOMAIN=messenger.yourdomain.com

# 2. Start services with the production Compose profile:
docker compose -f compose.yaml -f compose.production.yaml up -d --build
```

---

## 3. Initial Administrator Account

The first account registered on a fresh deployment is automatically granted the administrator role.
- Administrators can manage system announcements, broadcast server notices, view account statistics, and create bot tokens.
- This assignment happens once and persists across restarts.

---

## 4. GitHub Release Webhooks & Auto-Updates

The server can automatically notify connected clients whenever a new official release is published on GitHub:

1. Configure `GITHUB_WEBHOOK_SECRET` with a secure random secret in `.env`.
2. In your GitHub repository settings, add a webhook:
   - **Payload URL**: `https://YOUR_DOMAIN/v1/webhooks/github`
   - **Content type**: `application/json`
   - **Secret**: The exact secret configured in `GITHUB_WEBHOOK_SECRET`.
   - **Events**: Select **Releases**.
3. When a release is published on GitHub:
   - The backend validates the HMAC-SHA256 signature.
   - The release metadata is recorded in the database.
   - Connected desktop and mobile clients receive real-time notifications over WebSocket and can download the update from `/v1/releases/latest`.

---

## 5. Backup & Maintenance

### PostgreSQL Database Backup
```sh
docker compose exec postgres pg_dump -U nmessenger nmessenger > nmessenger_backup_$(date +%F).sql
```

### PostgreSQL Database Restore
```sh
cat nmessenger_backup.sql | docker compose exec -T postgres psql -U nmessenger nmessenger
```

### Preserving Uploaded Attachments
The `UPLOAD_DIR` directory stores all encrypted user attachments. When performing backups or migrating hosts, include the `data/uploads` volume alongside the SQL dump.

