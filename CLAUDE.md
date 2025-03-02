# CLAUDE.md - Guidelines for Fee Analyzer

## Build & Run Commands
- Build: `cargo build`
- Release build: `cargo build --release`
- Run: `cargo run -- <WALLET_ADDRESS> <HOURS_TO_LOOK_BACK> [RPC_ENDPOINT]`
- Check: `cargo check`
- Format: `cargo fmt`
- Clippy: `cargo clippy`
- Test: `cargo test`
- Test specific: `cargo test <test_name>`

## Code Style
- Use Rust 2021 edition
- Follow standard Rust naming conventions (snake_case for variables/functions)
- Organize imports alphabetically, group by source (std, external crates)
- Error handling: Use Result with meaningful error messages
- Use strong typing with structs for data organization
- Document public functions and structs with rustdoc comments
- Prefer async/await over manual Future handling
- Handle RPC rate limits with appropriate delays
- Format code with `cargo fmt` before committing
- Use `Box<dyn Error>` for general error handling