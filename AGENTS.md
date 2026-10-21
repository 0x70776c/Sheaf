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
Io, InvalidPdf, Encrypted, WrongPassword, PageOutOfRange, EmptySelection, InvalidSelection, OutputExists, Unsupported, Cancelled

## CLI-Vertrag (verbindlich für jedes Feature)

- Jedes Feature ist per CLI nutzbar, bevor es eine UI bekommt. Kein Feature ohne Subcommand.
- Nie interaktiv: keine Prompts, keine Rückfragen. Fehlende Infos = Fehler + Exit-Code.
- stdout = ausschließlich Ergebnis (bei --json genau EIN JSON-Objekt). stderr = Logs, Fortschritt, Warnungen.
- Jeder Befehl unterstützt: --json, --dry-run, --force, --quiet, --progress, --no-color.
- Ausgabedateien werden atomisch geschrieben (temp-Datei im Zielordner, dann rename).
- Bestehende Ausgabedatei ohne --force = Fehler OutputExists (Exit 7). Eingabedatei wird nie überschrieben.
- Exit-Codes und JSON-Fehlercodes sind stabil (Tabelle in SHEAF_MILESTONE_2.md). Änderungen nur mit Schema-Version-Bump.
- JSON-Envelope trägt "schema": 1. Felder werden nur hinzugefügt, nie umbenannt/entfernt.
- Jeder Subcommand: --help mit mindestens zwei Beispielen (clap `after_help`).
- Result-/Report-Structs liegen in sheaf-core (derive Serialize), die CLI serialisiert sie nur.
- Neues Feature = neuer Subcommand + CLI-Integrationstest (assert_cmd) + Eintrag in `sheaf capabilities`.

## Secrets (verschärft)
- Passwörter nie als CLI-Argument. Nur `--*-password-env`, `--*-password-file`, `--*-password-stdin`.
- Passwörter nur als `secrecy::SecretString`. Kein `Debug`/`Display`, kein Logging, nicht in JSON, Fehlern oder Snapshots.
- Höchstens EIN `--*-stdin` pro Aufruf.
- Entschlüsselte Daten werden nie in Temp-Dateien außerhalb des Zielordners geschrieben und nie im Disk-Cache abgelegt.

## PDF laden
- Jede Operation lädt PDFs ausschließlich über `sheaf_core::load::load_pdf`.
- `lopdf::Document::load*` direkt aufrufen ist per `clippy.toml` (`disallowed-methods`) verboten.

## Dokumentation
- `docs/CLI.md` und `sheaf capabilities` werden aus der clap-Definition generiert, nicht von Hand gepflegt.
- Der Docs-Sync-Test schlägt fehl, wenn `docs/CLI.md` vom generierten Stand abweicht.
- Abschlussberichte enthalten rohe Tool-Ausgaben, keine Paraphrasen der CLI.


