# Development notes

- Run `cargo clippy --all-targets --all-features -- -D warnings` (or `cargo clippy-strict`) before committing to enforce the strict lint baseline. Clippy cannot be forced on every `cargo build`, so treat this as required preflight.
- Make invalid state unrepresentable (embrace ADTs)
- Parse, don't validate
- Push ifs up, push fors (loops) down
- Prefer immutability and functional programming style when you can without sacrificing code cleanliness
