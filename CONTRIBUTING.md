# Contributing to Sheaf

Thank you for your interest in contributing to Sheaf! We welcome bug reports, documentation improvements, and pull requests.

## Architecture & Codebase Guidelines

Sheaf is structured as a Cargo workspace:
- `crates/sheaf-core`: Pure PDF logic without any UI or Tauri dependencies.
- `crates/sheaf-cli`: Clap-based command line interface wrapping `sheaf-core`.
- `apps/desktop`: Tauri 2 desktop app with Svelte 5 and Tailwind CSS.
- `tests/fixtures`: Standard test PDF documents.

### Strict Rules
1. **100% Offline**: Sheaf must never make network connections or perform telemetry. Banned crates include `reqwest`, `hyper`, `tokio-tungstenite`, and `curl` (enforced by `cargo deny`).
2. **No Secret Leaks**: Passwords must always use `secrecy::SecretString` and must never be exposed via argv, debug logs, error messages, or JSON reports.
3. **Library Code Safety**: No `.unwrap()` or `.expect()` in `sheaf-core`. Errors are typed with `thiserror`.
4. **Code Quality**: `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` must pass cleanly.
5. **Atomic Writes**: Output files are always written to a temporary file in the destination folder first and atomically renamed. Original files are never modified in place.

## Local Development Setup

### Prerequisites
- Rust stable (1.85+ / Edition 2024)
- Node.js 20+ and npm
- `libpdfium.dylib` in `vendor/pdfium/lib/`
- (Optional) `qpdf` for deep verification tests (`brew install qpdf`)

### Running Checks & Tests
```bash
# Code formatting check
cargo fmt --all -- --check

# Clippy linter
cargo clippy --workspace --all-targets -- -D warnings

# License & banned dependency audit
cargo deny check licenses bans

# Run the test suite
cargo test --workspace

# Check Desktop UI
cd apps/desktop
npm ci
npm run check
npm run build
```

## Pull Request Process
1. Create a feature branch off `main` (`git checkout -b feat/your-feature`).
2. Make targeted commits with descriptive messages following Conventional Commits.
3. Ensure all tests and linter checks pass locally.
4. If modifying CLI flags or commands, update tests with `UPDATE_DOCS=1 cargo test -p sheaf-cli --test docs_sync_test` to keep `docs/CLI.md` synchronized.
5. Submit a Pull Request targeting `main`.
