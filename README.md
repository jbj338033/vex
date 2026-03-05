# vex

Open-source PaaS where AI agents deploy apps with a single CLI command.

No web UI — just a CLI and a JSON API. Self-hostable.

## Features

- **Auto-build** — detects project type (Node.js, Python, Go, Rust, Static) and generates a Dockerfile
- **One-line deploy** — `vex deploy ./my-app`
- **JSON API** — every response is JSON, ready for AI agents to parse
- **Self-host** — single binary + PostgreSQL + Docker
- **Built-in proxy** — host-based reverse proxy included (no Nginx/Traefik needed)
- **Auto TLS** — per-app certificates via Let's Encrypt HTTP-01 (no DNS provider required)

## Install

### From GitHub Releases

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/jbj338033/vex/releases/latest/download/vex-darwin-arm64 -o vex
chmod +x vex && sudo mv vex /usr/local/bin/

# macOS (Intel)
curl -fsSL https://github.com/jbj338033/vex/releases/latest/download/vex-darwin-amd64 -o vex
chmod +x vex && sudo mv vex /usr/local/bin/

# Linux (amd64)
curl -fsSL https://github.com/jbj338033/vex/releases/latest/download/vex-linux-amd64 -o vex
chmod +x vex && sudo mv vex /usr/local/bin/

# Linux (arm64)
curl -fsSL https://github.com/jbj338033/vex/releases/latest/download/vex-linux-arm64 -o vex
chmod +x vex && sudo mv vex /usr/local/bin/
```

### From source

```bash
cargo install --git https://github.com/jbj338033/vex vex-cli
```

## Quick Start

### Run the server

```bash
docker compose up -d
```

### Log in

```bash
vex login --url http://localhost:3000
```

Opens GitHub browser auth; an API key is issued automatically.

### Deploy

```bash
vex deploy ./my-app --name my-app
```

### Check status

```bash
vex status my-app
```

## CLI

```
vex login [--url <server-url>]                 GitHub login
vex deploy <path> [--name <name>]              Deploy a directory
vex deploy --git <url> --name <name>           Deploy from Git URL
vex logs <app> [-f] [-n 100]                   View logs (-f: stream)
vex env list <app>                             List env vars
vex env set <app> KEY=VALUE [KEY2=VALUE2...]    Set env vars
vex env unset <app> KEY [KEY2...]              Unset env vars
vex status <app>                               App status
vex destroy <app> --force                      Delete app
```

All commands support `--format json` (default) or `--format text`.

## API

Base URL: `http://localhost:3000/v1`

Auth: `Authorization: Bearer vex_xxx`

### Public

| Method | Path | Description |
|--------|------|-------------|
| POST | `/auth/device-code` | Start GitHub Device Flow |
| POST | `/auth/device-token` | Exchange device token |

### Authenticated

| Method | Path | Description |
|--------|------|-------------|
| POST | `/apps` | Create app |
| GET | `/apps` | List apps |
| GET | `/apps/:name` | Get app |
| DELETE | `/apps/:name` | Delete app |
| POST | `/apps/:name/deploy` | Deploy (multipart) |
| GET | `/apps/:name/deployments` | Deployment history |
| GET | `/apps/:name/logs` | Logs |
| GET | `/apps/:name/logs/stream` | SSE log stream |
| GET | `/apps/:name/env` | List env vars |
| PUT | `/apps/:name/env` | Set env vars |
| DELETE | `/apps/:name/env/:key` | Delete env var |
| GET | `/apps/:name/status` | App status |

### Response format

```json
{"ok": true, "data": {...}}
{"ok": false, "error": {"code": "app_not_found", "message": "..."}}
```

## Supported Projects

| File | Type | Details |
|------|------|---------|
| `Dockerfile` | Custom | Used as-is |
| `package.json` | Node.js | pnpm/yarn/bun/npm, Next.js/Vite/Remix |
| `pyproject.toml` | Python | uv/poetry |
| `requirements.txt` | Python | pip |
| `go.mod` | Go | — |
| `Cargo.toml` | Rust | — |
| `index.html` | Static | Nginx |

If a `Dockerfile` exists, auto-detection is skipped.

## Self-Hosting

### Requirements

- Docker
- PostgreSQL 15+
- Wildcard DNS (e.g. `*.vex.dev` → server IP)
- GitHub App with Device Flow enabled

### GitHub App setup

1. Create a new GitHub App at [Developer Settings](https://github.com/settings/apps/new)
2. Enable "Device Flow"
3. Set `GITHUB_CLIENT_ID` and `GITHUB_CLIENT_SECRET` env vars

### TLS (optional)

Enable automatic per-app TLS certificates via Let's Encrypt HTTP-01:

```bash
VEX_TLS_ENABLED=true
VEX_ACME_EMAIL=admin@example.com
VEX_CERT_DIR=/var/lib/vex/certs
```

Ports when TLS is enabled:
- **443** — HTTPS proxy (main traffic)
- **80** — ACME challenge + HTTP→HTTPS redirect

When TLS is disabled (default), the proxy listens on a single HTTP port (default `8080`).

### Docker Compose

```bash
docker compose up -d
```

### Manual

```bash
export DATABASE_URL=postgres://vex:vex@localhost:5432/vex
export VEX_DOMAIN=vex.dev
vex-server
```

## Architecture

```
vex/
├── crates/
│   ├── vex-core/       # Shared types, DB models, errors, schemas
│   ├── vex-builder/    # Project detection + Dockerfile generation
│   ├── vex-server/     # axum API server + reverse proxy + TLS
│   └── vex-cli/        # clap CLI
├── migrations/         # PostgreSQL migrations
├── compose.yaml
└── Dockerfile.server
```

### Stack

| Purpose | Crate |
|---------|-------|
| HTTP server | axum |
| Docker API | bollard |
| Database | sqlx + PostgreSQL |
| CLI | clap |
| HTTP client | reqwest |
| Runtime | tokio |
| Route table | dashmap |
| TLS / ACME | instant-acme + rustls |

## Development

```bash
cargo build                      # Build
cargo test                       # Test
cargo clippy -- -D warnings      # Lint
cargo fmt --check                # Format check
```

## License

MIT
