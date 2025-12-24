# Contributing to Zagel

Thanks for helping improve Zagel. The project is early-stage and feedback is
welcome, especially around UI polish and documentation.

## Quick start

1. Fork the repo and create a topic branch.
2. Install a recent Rust toolchain (`stable`) and `cargo`.
3. Run the app locally:

```bash
cargo run
```

## Linting (required)

Run clippy with warnings as errors before opening a PR:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

If you have it configured, `cargo clippy-strict` is equivalent.

## Tests

If your change adds logic, run:

```bash
cargo test
```

## Docs and UX updates

Documentation and UI improvements are welcome. If you change user-facing
behavior or the UI, update `README.md` and screenshots as needed.

## PR checklist

- Keep the scope focused and explain the motivation.
- Note any manual testing performed.
- Confirm clippy passes.
