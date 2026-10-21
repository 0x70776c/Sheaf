# Sheaf

A fast, minimalist, **100% offline** PDF toolkit for macOS (Apple Silicon & Intel).
No cloud uploads, no accounts, no telemetry. Original files are never modified.

---

## Features & Architecture

```
crates/
├─ sheaf-core   # Pure PDF manipulation library (lopdf), zero UI dependencies
└─ sheaf-cli    # Agent- and script-friendly CLI (`sheaf`) with JSON envelope & atomic writes
tests/
└─ fixtures     # Curated and programmatically generated PDF test fixtures
```

### Core Principles
- **100% Offline & Private:** Zero network calls, zero telemetry.
- **Non-Destructive:** Input files are never overwritten; outputs are written atomically via temporary files in the target directory.
- **Deterministic CLI Contract:** Every command supports `--json` (emitting a single schema-versioned JSON object on `stdout`), `--dry-run`, `--force`, `--quiet`, `--progress <none|human|json>`, and `--no-color`.

---

## Quick Start & Setup

### Prerequisites
- **Rust** (stable toolchain, 1.85+ / Edition 2024)
- **qpdf** *(optional, used for deep PDF structural validation via `sheaf verify --deep` and test suites)*:
  ```bash
  brew install qpdf
  ```

### Build & Run
```bash
# Clone the repository
git clone https://github.com/0x70776c/Sheaf.git
cd Sheaf

# Build release binary
cargo build --release -p sheaf-cli

# Inspect CLI capabilities
./target/release/sheaf capabilities --json
```

---

## CLI Reference

### Global Flags

Available on every subcommand:

| Flag | Description |
|---|---|
| `--json` | Emit a single JSON envelope (`"schema": 1`) to `stdout` |
| `--dry-run` | Validate inputs and compute execution plan without writing any files |
| `--force` | Overwrite existing output files (input files are never overwritten) |
| `-q, --quiet` | Suppress human-readable output on `stdout` (except errors on `stderr`) |
| `-v, --verbose` | Emit verbose debug logs to `stderr` |
| `--progress <none\|human\|json>` | Progress reporting on `stderr` (`json` emits NDJSON events) |
| `--no-color` | Disable colored console output (also respects `NO_COLOR`) |

---

### 1. `sheaf merge`
Merge multiple PDF files into a single output PDF. Preserves page order and merges existing bookmarks/outlines (including hierarchy levels).

```bash
sheaf merge part1.pdf part2.pdf part3.pdf -o combined.pdf
sheaf merge a.pdf b.pdf -o combined.pdf --force --json
```

---

### 2. `sheaf split`
Split a PDF document by page ranges (`--ranges`), fixed page chunks (`--every`), or bookmark hierarchy levels (`--bookmarks-level`).

**Page Range Syntax:** `3` (single page), `1-3` (closed range), `8-` (to end), `-3` (from page 1), `1-3,5,8-` (multiple ranges; overlaps and duplicates allowed).

```bash
# One output file per range token ({stem}_p1-3.pdf, {stem}_p5.pdf, ...)
sheaf split report.pdf --ranges "1-3,5,8-" -o out/

# Combine all selected ranges into a single file ({stem}_selection.pdf)
sheaf split report.pdf --ranges "1-3,8-" --combine -o out/

# Split every N pages ({stem}_part01.pdf, {stem}_part02.pdf, ...)
sheaf split report.pdf --every 2 -o out/

# Split by top-level bookmarks ({stem}_01_chapter-slug.pdf, ...)
sheaf split report.pdf --bookmarks-level 1 -o out/ --json
```

---

### 3. `sheaf pages`
Page-level operations: `rotate`, `delete`, `reorder`, and `extract`.

```bash
# Rotate selected pages (or omit --pages to rotate all pages) additively by 90, 180, 270, or -90 degrees
sheaf pages rotate report.pdf --pages "1-3" --degrees 90 -o rotated.pdf

# Delete selected pages (at least 1 page must remain)
sheaf pages delete report.pdf --pages "2,5-7" -o cleaned.pdf

# Reorder pages according to a page range sequence
sheaf pages reorder report.pdf --order "3,1,2,4-" -o reordered.pdf

# Extract selected pages into a new PDF file
sheaf pages extract report.pdf --pages "1,4-6" -o subset.pdf --json
```

---

### 4. `sheaf info`
Inspect structural properties and metadata (`size_bytes`, `pdf_version`, `pages`, `encrypted`, `metadata`, `has_outlines`, `outline_count`, `first_page_size_pt`). Encrypted PDFs return exit code `0` with `"encrypted": true` and `"pages": null`.

```bash
sheaf info report.pdf
sheaf info a.pdf b.pdf --json
```

---

### 5. `sheaf verify`
Verify PDF files for structural integrity (loads cleanly, valid catalog & page tree, resolvable page content streams and resources). With `--deep`, also runs `qpdf --check` when available.

```bash
sheaf verify report.pdf
sheaf verify out/*.pdf --deep --json
```

---

### 6. `sheaf capabilities`
Self-description of CLI version, schema version, supported commands, exit codes, and external tool availability (`qpdf`).

```bash
sheaf capabilities --json
```

---

## Exit Codes & JSON Error Contract

When `--json` is passed, `stdout` always contains a single valid JSON object—even on errors or CLI usage mistakes:

```json
{
  "schema": 1,
  "ok": false,
  "op": "split",
  "error": {
    "code": "page_out_of_range",
    "exit_code": 5,
    "message": "Page 99 is out of range (document has 12 pages)",
    "hint": "Valid page range: 1-12."
  }
}
```

| Exit Code | JSON `error.code` | Description |
|---|---|---|
| `0` | – | Success |
| `1` | `internal` | Unexpected internal error |
| `2` | `invalid_selection` / `empty_selection` | Invalid CLI arguments or invalid/empty page selection |
| `3` | `invalid_pdf` | Corrupted or invalid PDF document |
| `4` | `encrypted` / `wrong_password` | PDF is password-protected or password is wrong |
| `5` | `page_out_of_range` | Requested page number is `0` or exceeds document page count |
| `6` | `io` | File system I/O error (e.g. file not found, permission denied) |
| `7` | `output_exists` | Output file already exists and `--force` was not specified |
| `8` | `unsupported` | Unsupported feature (e.g. splitting by bookmarks on a PDF without bookmarks) |
| `130` | `cancelled` | Operation cancelled (`SIGINT` / `Ctrl+C`) |

---

## Development & Testing

```bash
# Format check
cargo fmt --all -- --check

# Lint with Clippy (warnings denied)
cargo clippy --workspace --all-targets -- -D warnings

# Run all unit, property (proptest), snapshot (insta), and CLI integration tests
cargo test --workspace
```

## License

Dual-licensed under **MIT** or **Apache-2.0**.
