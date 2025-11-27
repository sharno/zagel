# Development notes

- Run `cargo clippy --all-targets --all-features -- -D warnings` (or `cargo clippy-strict`) before committing to enforce the strict lint baseline. Clippy cannot be forced on every `cargo build`, so treat this as required preflight. 
