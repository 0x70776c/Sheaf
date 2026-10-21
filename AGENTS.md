# AGENTS.md – Sheaf

## Stack
- Rust (stable), Cargo Workspace
- Core: lopdf (Manipulation), pdfium-render (Rendering)
- UI: Tauri 2 + Svelte 5 + TypeScript + Vite
- CLI: clap
- Tests: cargo test, insta (Snapshots), proptest (Properties)

## Struktur
crates/sheaf-core   -> reine Logik, KEINE UI-Abhängigkeiten
crates/sheaf-cli    -> CLI-Wrapper um core
apps/desktop        -> Tauri-App (src-tauri + web)
tests/fixtures      -> Test-PDFs (normal, kaputt, verschlüsselt, 500+ Seiten, Formulare)

## Regeln
- `cargo clippy -- -D warnings` und `cargo fmt --check` müssen passen.
- Kein `unwrap()`/`expect()` in Library-Code. Fehler via `thiserror`.
- Öffentliche API in sheaf-core ist dokumentiert (`///`) mit Beispiel.
- Jede Operation: `fn op(input, options, progress) -> Result<Output, SheafError>`.
- Lange Operationen laufen auf Worker-Threads (rayon/tokio), melden Fortschritt über Channel.
- Keine Netzwerkzugriffe. Keine Telemetrie.
- Originaldateien nie verändern.

## Fehlerklassen (SheafError)
Io, InvalidPdf, Encrypted, WrongPassword, PageOutOfRange, EmptySelection, Unsupported, Cancelled
