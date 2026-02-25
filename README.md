# Zagel

Zagel is a cross-platform, GUI REST workbench built with Rust + `iced`. It scans your workspace for request collections (`.http`) and environments (`.env`), lets you pick a request, edit it, and send it. Think of it as a workspace-native alternative to Postman or Insomnia for teams that already keep requests in `.http` files.

The UI is still rough around the edges but it's functioning with good keyboard shortcuts.
<img width="2468" height="1440" alt="image" src="https://github.com/user-attachments/assets/962da7f6-1933-45c1-9a77-0f1181732147" />

## Features

- GUI request composer (method, URL, headers, body)
- Auth helpers: Bearer, API key, Basic, OAuth2 client credentials
- Loads requests from `.http` files (blocks separated by `###`)
- Loads environments from `.env` files (simple `KEY=VALUE` format)
- Variable substitution in URL/headers/body via `{{VAR_NAME}}`
- Add/remove multiple project roots from the sidebar
- Per-project environment files plus optional global environment roots
- Periodic rescan of configured folders

## Install

Download a prebuilt binary from GitHub Releases (recommended), or build from source (below).

## Build & run from source

```bash
cargo run
```

### Automation mode (UI actions + screenshots)

You can run scripted UI flows for repeatable testing and screenshot capture:

```bash
cargo run -- \
  --state-file ./.tmp/zagel-state.toml \
  --project-root ./tests/ui/fixtures/workspace \
  --automation ./tests/ui/scenarios/smoke.toml \
  --screenshot-dir ./artifacts/ui \
  --automation-state-out ./artifacts/ui/state.json \
  --exit-when-done
```

When `--automation-state-out` is provided, Zagel writes a full JSON snapshot of
runtime/app/workspace state at the end of the run (both success and failure), so
E2E tests can assert behavior without parsing logs.

Supported scenario actions:
- `select_request` (`value = "relative/path.http#0"`)
- `send`
- `wait_for_status` (`value = 200` or `"200"`)
- `wait_for_text` (`value = "Received response"`)
- `wait_for_millis` (`value = 1000`)
- `screenshot` (`value = "after-send"`)

Each action is declared as a `[[step]]` block in TOML.

Bundled scenarios in `tests/ui/scenarios/`:
- `smoke.toml`: basic selection, send, and screenshot flow
- `ui_navigation.toml`: selection + ready-state screenshots (no network dependency)
- `rest_send_status.toml`: send + `wait_for_status` + body text assertion
- `snapshot_only.toml`: minimal flow for state snapshot generation

Run E2E automation test locally (opt-in):

```bash
ZAGEL_E2E=1 cargo test --test e2e_automation -- --nocapture
```

Release build:

```bash
cargo build --release
```

## Nix / nixci

This repo now includes a `flake.nix` so nix-based CI can run `nix flake check`
directly.

Local verification:

```bash
nix flake check
```

For an interactive shell with Rust + Linux GUI deps:

```bash
nix develop
cargo test --locked
```

### WSL setup (Ubuntu)

If you want local parity on Windows, install Nix in WSL and run checks there:

```bash
wsl -d Ubuntu
curl -fsSL https://install.determinate.systems/nix | sh -s -- install
cd /mnt/c/Users/<your-user>/projects/zagel
nix flake check
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

Zagel stores UI/application state in `~/.config/zagel/state.toml` (exact location depends on your OS).

Relevant keys:
- `project_roots` (folders scanned for `.http` request collections and project-scoped `.env` files)
- `global_env_roots` (folders scanned for global `.env` files)
- `active_environment` (last selected environment label)

## Contributing

Contributions are welcome. See `CONTRIBUTING.md` for setup, linting, and the suggested workflow.
