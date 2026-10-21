# Sheaf

Schnelles, minimalistisches und **100 % offline** arbeitendes PDF-Toolkit für macOS (Apple Silicon + Intel).
Keine Cloud, keine Accounts, keine Telemetrie. Originaldateien werden niemals verändert.

## Struktur

- `crates/sheaf-core` – Reine PDF-Verarbeitungslogik (`lopdf`), keine UI-Abhängigkeiten
- `crates/sheaf-cli` – Agent- und Skript-taugliches CLI (`sheaf`) um `sheaf-core`
- `tests/fixtures` – Kuratierte und programmatisch erzeugte Test-PDFs

## Globale CLI-Flags

Jeder Subcommand unterstützt:

| Flag | Wirkung |
|---|---|
| `--json` | `stdout` gibt genau ein JSON-Objekt (Schema-Version `1`) aus |
| `--dry-run` | Validiert Eingaben und berechnet den Ausführungsplan, schreibt aber keine Dateien |
| `--force` | Vorhandene Ausgabedateien überschreiben (Eingabedateien werden nie überschrieben) |
| `-q, --quiet` | Keine Human-Ausgabe außer Fehlern |
| `-v, --verbose` | Erweiterte Debug-Ausgaben auf `stderr` |
| `--progress <none\|human\|json>` | Fortschrittsausgabe auf `stderr` (`json` = NDJSON-Events) |
| `--no-color` | Deaktiviert farbige Ausgabe (respektiert auch `NO_COLOR`) |

## Unterstützte Befehle

### `sheaf merge`
Führt mehrere PDF-Dateien in der angegebenen Reihenfolge zu einer neuen PDF-Datei zusammen. Seitenbäume und Lesezeichen (Outlines/Bookmarks) inklusive Verschachtelung bleiben erhalten.

```bash
sheaf merge a.pdf b.pdf c.pdf -o merged.pdf
sheaf merge a.pdf b.pdf -o merged.pdf --force --json
```

### `sheaf split`
Teilt eine PDF-Datei nach Seitenbereichen (`--ranges`), festen Blockgrößen (`--every`) oder Lesezeichen-Ebenen (`--bookmarks-level`) auf.

```bash
sheaf split report.pdf --ranges "1-3,5,8-" -o out/
sheaf split report.pdf --ranges "1-3,8-" --combine -o out/
sheaf split report.pdf --every 2 -o out/
sheaf split report.pdf --bookmarks-level 1 -o out/ --json
```

### `sheaf info`
Zeigt Struktur- und Metadaten-Informationen (Seitenzahl, PDF-Version, Verschlüsselung, Metadaten, Lesezeichen, Seitengröße) an.

```bash
sheaf info report.pdf
sheaf info a.pdf b.pdf --json
```

### `sheaf verify`
Prüft PDF-Dateien auf strukturelle Integrität (und mit `--deep` zusätzlich via `qpdf --check`).

```bash
sheaf verify report.pdf
sheaf verify out/*.pdf --deep --json
```

### `sheaf capabilities`
Selbstbeschreibung aller unterstützten Subcommands, Argumente, Exit-Codes und verfügbaren System-Tools (`qpdf`).

```bash
sheaf capabilities --json
```

## Exit-Codes & JSON-Fehlercodes

| Exit-Code | JSON `error.code` | Bedeutung |
|---|---|---|
| `0` | – | Erfolg |
| `1` | `internal` | Unerwarteter interner Fehler |
| `2` | `invalid_selection` / `empty_selection` | Ungültige CLI-Argumente oder ungültige/leere Seitenauswahl |
| `3` | `invalid_pdf` | Ungültige oder beschädigte PDF-Datei |
| `4` | `encrypted` / `wrong_password` | PDF ist passwortgeschützt oder Passwort falsch |
| `5` | `page_out_of_range` | Angeforderte Seite liegt außerhalb von `1..=total` |
| `6` | `io` | Dateisystem-/Zugriffsfehler (z. B. Datei nicht gefunden) |
| `7` | `output_exists` | Ausgabedatei existiert bereits und `--force` wurde nicht gesetzt |
| `8` | `unsupported` | Nicht unterstütztes Feature (z. B. keine Lesezeichen auf der gewünschten Ebene) |
| `130` | `cancelled` | Vorgang abgebrochen (`Ctrl+C`) |

## Entwicklung

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
