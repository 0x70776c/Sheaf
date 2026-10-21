# Sheaf

Schnelles, minimalistisches und **100 % offline** arbeitendes PDF-Toolkit für macOS (Apple Silicon + Intel).
Keine Cloud, keine Accounts, keine Telemetrie. Originaldateien werden niemals verändert.

## Struktur

- `crates/sheaf-core` – Reine PDF-Verarbeitungslogik (`lopdf`), keine UI-Abhängigkeiten
- `crates/sheaf-cli` – Schlankes CLI (`sheaf`) um `sheaf-core`
- `tests/fixtures` – Kuratierte und programmatisch erzeugte Test-PDFs

## Unterstützte Features

### Merge (`sheaf merge`)

Führt mehrere PDF-Dateien in der angegebenen Reihenfolge zu einer neuen PDF-Datei zusammen. Seitenbäume und Lesezeichen (Outlines/Bookmarks) inklusive Verschachtelung bleiben erhalten.

```bash
sheaf merge a.pdf b.pdf c.pdf -o merged.pdf
```

## Entwicklung

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
