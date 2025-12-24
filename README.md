# Zagel

Zagel is a cross-platform, GUI REST workbench built with Rust + `iced`. It scans your workspace for request collections (`.http`) and environments (`.env`), lets you pick a request, edit it, and send it.

The UI is still rough around the edges but it's functioning with good keyboard shortcuts.
<img width="2468" height="1440" alt="image" src="https://github.com/user-attachments/assets/962da7f6-1933-45c1-9a77-0f1181732147" />

## Features

- GUI request composer (method, URL, headers, body)
- Loads requests from `.http` files (blocks separated by `###`)
- Loads environments from `.env` files (simple `KEY=VALUE` format)
- Variable substitution in URL/headers/body via `{{VAR_NAME}}`
- Periodic rescan of your configured roots

## Install

Download a prebuilt binary from GitHub Releases (recommended), or build from source (below).

## Build & run from source

```bash
cargo run
```

Release build:

```bash
cargo build --release
```

## `.http` file format

Zagel treats each request as a block. Blocks are separated by lines starting with `###`.

Example `requests.http`:

```http
GET https://httpbin.org/get
Accept: application/json

###
POST https://httpbin.org/post
Content-Type: application/json

{"hello":"world"}
```

Rules:
- First non-empty line: `METHOD URL`
- Subsequent non-empty lines until the first blank line: headers (`Name: Value`)
- After the blank line: body (optional)

## `.env` file format

Any file whose name ends with `.env` is treated as an environment. Format is `KEY=VALUE` per line; `#` starts a comment.

Example `dev.env`:

```env
API_URL=https://httpbin.org
TOKEN=secret
```

You can use variables in requests as `{{API_URL}}` / `{{TOKEN}}`.

## Configuration

Zagel stores settings in `~/.config/zagel/config.toml` (exact location depends on your OS).

Supported keys:
- `http_root` (directory to scan for `.http`)
- `env_root` (directory to scan for `*.env`, defaults to `http_root`)
- `polling_interval_secs` (default `2`)
- `scan_depth` (default `6`)
