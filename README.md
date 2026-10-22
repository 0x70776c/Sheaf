# Sheaf

A fast, minimalist, **100% offline** PDF toolkit for macOS (Apple Silicon & Intel).
No cloud uploads, no accounts, no telemetry. Original files are never modified.

---

## Features & Architecture

```
crates/
├─ sheaf-core   # Pure PDF manipulation & rendering library (lopdf, pdfium-render), zero UI dependencies
└─ sheaf-cli    # Agent- and script-friendly CLI (`sheaf`) with JSON envelope & atomic writes
apps/
└─ desktop      # Native macOS desktop app (Tauri 2 + Svelte 5 + TypeScript + Vite)
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
- **PDFium (`libpdfium.dylib`)** *(used for page rendering and thumbnail generation via `pdfium-render`)*:
  ```bash
  mkdir -p vendor/pdfium
  curl -sSL https://github.com/bblanchon/pdfium-binaries/releases/latest/download/pdfium-mac-arm64.tgz \
    | tar -xz -C vendor/pdfium
  ```
  *(You can also point `PDFIUM_LIB_PATH` to any custom `libpdfium.dylib` location.)*
- **qpdf** *(optional, used for deep PDF structural validation via `sheaf verify --deep` and test suites)*:
  ```bash
  brew install qpdf
  ```

### Build & Run
```bash
# Clone the repository
git clone https://github.com/0x70776c/Sheaf.git
cd Sheaf

# Build CLI release binary
cargo build --release -p sheaf-cli

# Inspect CLI capabilities
./target/release/sheaf capabilities --json

# Typecheck & build the Svelte 5 desktop frontend
cd apps/desktop
npm install
npm run check
npm run build
```

### Desktop Application (`apps/desktop`)
- **Minimalist macOS Design:** Transparent overlay titlebar, 220 px vibrancy sidebar with Lucide icons (`stroke-width: 1.5`), warm light (`#FAFAF9`) and native dark (`#161617`) color tokens, and 4 px grid spacing.
- **Interactive Thumbnail Grid & Progressive Disclosure:**
  - Lazy-loaded PDFium page thumbnails with skeleton placeholders and bi-directional page range synchronization.
  - Multi-page selection via single click, `⇧+Click` (range), `⌘+Click` (toggle), and `⌘A` (select/deselect all).
  - Dedicated right-hand options panels for **Merge** (queue reordering, add/remove files), **Split** (*Ranges* with *Combine* toggle, *Every N*, *Bookmarks*), **Pages** (*Rotate* 90°/180°/270°, *Delete*, *Reorder*, *Extract*), **Compress** (*Low* / *Medium* / *High* presets + *Grayscale* toggle with before/after size comparison), **Convert** (*PDF → Images*, *Images → PDF*, *PDF → Markdown*, and *Markdown → PDF*), **Watermark** (text, placement, font size, opacity, color, page selection), **Protect** (encrypt with user/owner passwords & permissions or unlock password-protected PDFs), and **Info**.
  - Real-time progress bar with cancellation (`Cancel`) and completion toast with **Show in Finder** (`open -R`) and **Open** actions.
- **Keyboard-First Workflow:**
  - `⌘K`: Open Command Palette to switch tools, open files, or toggle theme (`System` / `Light` / `Dark`)
  - `⌘O`: Open PDF file dialog
  - `⌘A`: Select or deselect all pages in the thumbnail grid
  - `⌘1`–`⌘8`: Jump directly to *Merge*, *Split*, *Pages*, *Compress*, *Convert*, *Watermark*, *Protect*, or *Info*
  - `↑` / `↓` / `Enter` / `Esc`: Full keyboard navigation across the sidebar, drop-zone, and command palette.

---

## CLI Reference

> For complete generated command-line reference documentation, see [docs/CLI.md](docs/CLI.md).

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

### 4. `sheaf render`
Render PDF pages to PNG images or thumbnails using PDFium (`pdfium-render`).
When rendering by `--width`, rendered thumbnails are automatically cached on disk keyed by `(path, mtime, size, page, width_px)` (disable with `--no-cache`).

```bash
# Render a single page at 150 DPI to a PNG file
sheaf render report.pdf --page 1 --dpi 150 -o page1.png

# Render the first 20 pages as 160 px wide thumbnails into a directory
sheaf render report.pdf --pages "1-20" --width 160 -o thumbs/ --json
```

---

### 5. `sheaf compress`
Compress a PDF file by resampling embedded Image XObjects, deduplicating identical streams, and compacting PDF structure—while keeping all vector text 100% selectable.
If a document cannot be reduced further, original bytes are preserved and `"already_optimal": true` is reported.

**Presets:**
- `low`: 220 DPI target, JPEG quality 85, structural cleanup
- `medium` *(default)*: 150 DPI target, JPEG quality 70, stream deduplication + resampling
- `high`: 100 DPI target, JPEG quality 55, stream deduplication + aggressive resampling

```bash
# Compress with the default medium preset (150 DPI, JPEG 70)
sheaf compress scan.pdf -o scan_compressed.pdf

# High compression with grayscale conversion and JSON output
sheaf compress photo_report.pdf --preset high --grayscale -o compact.pdf --json
```

---

### 6. `sheaf convert`
Convert between PDF, images (`PNG`/`JPG`), and Markdown (`.md`) documents:
- `to-images`: Export all or selected PDF pages as `PNG` or `JPG` at custom `--dpi` (`18..=1200`) and JPEG `--quality` (`1..=100`).
- `from-images`: Combine `PNG`/`JPG` images into a multi-page PDF (`--page-size fit|a4|letter` and `--margin` in PDF points).
- `to-markdown`: Extract structured Markdown (`.md`) from a PDF (reconstructing headings from bookmarks and font sizes, bullet lists, code blocks, and paragraphs).
- `from-markdown`: Render a Markdown (`.md`) document into a paginated vector PDF (`--page-size a4|letter`, `--margin`) with selectable text and automatic PDF bookmarks/outlines for headings.

```bash
# Export PDF pages as 150 DPI PNG images
sheaf convert to-images report.pdf --format png --dpi 150 -o images/

# Combine PNG/JPG images into an A4 PDF with 36 pt margins
sheaf convert from-images page1.png page2.jpg --page-size a4 --margin 36 -o scan.pdf

# Extract structured Markdown (.md) from a PDF
sheaf convert to-markdown report.pdf -o report.md

# Render a Markdown (.md) file into a multi-page vector PDF with outlines
sheaf convert from-markdown README.md --page-size a4 --margin 54 -o README.pdf --json
```

---

### 7. `sheaf watermark`
Stamp a customizable vector text watermark onto all or selected PDF pages with configurable placement (`diagonal`, `center`, `top-left`, `top-right`, `bottom-left`, `bottom-right`), `--font-size`, `--opacity` (`0.05..=1.0`), `--rotation`, and `--color` (`#RRGGBB` hex or `gray`/`red`/`blue`/`black`/`green`).

```bash
# Diagonal watermark across all pages
sheaf watermark report.pdf --text "CONFIDENTIAL" -o report_watermarked.pdf

# Bottom-right red watermark on selected pages
sheaf watermark draft.pdf --text "DRAFT" --position bottom-right --color red --opacity 0.35 --pages "1-3" -o out.pdf --json
```

---

### 8. `sheaf number`
Stamp vector page numbers onto PDF pages with format placeholders (`{n}`, `{total}`, `{page}`, `{pages}`), placement presets, start number, font size, color, opacity (`--opacity`, 0.05..=1.0), and page selection:

```bash
# Bottom-center page numbering across all pages (default format: {n})
sheaf number report.pdf -o report_numbered.pdf

# Bottom-right numbering starting at page 2 with opacity, skipping cover page
sheaf number thesis.pdf --format "Page {n} of {total}" --position bottom-right --start 1 --pages "2-" --opacity 0.8 -o thesis_numbered.pdf --json
```

---

### 9. `sheaf protect` & `sheaf unlock`
Encrypt (`protect`) a PDF with user/owner passwords and print/copy/modify permissions, or decrypt (`unlock`) an encrypted PDF (returns exit code `4` with `"code": "wrong_password"` if the password is incorrect). Passwords are never passed via command-line arguments, but via environment variables, files, or stdin.

> **Hinweis:** `--no-print`, `--no-copy`, `--no-modify` sind Hinweise an den PDF-Viewer. Manche Programme ignorieren sie. Echter Schutz entsteht nur durch das User-Passwort.

```bash
# Encrypt a PDF with user and owner passwords from environment variables
sheaf protect contract.pdf --new-password-env PDF_PASS --new-owner-password-env OWNER_PASS --no-copy -o locked.pdf

# Unlock an encrypted PDF using standard input
printf '%s' "$PDF_PASS" | sheaf unlock locked.pdf --password-stdin -o unlocked.pdf --json
```

---

### 10. `sheaf info`
Inspect structural properties and metadata (`size_bytes`, `pdf_version`, `pages`, `encrypted`, `metadata`, `has_outlines`, `outline_count`, `first_page_size_pt`). Encrypted PDFs return exit code `0` with `"encrypted": true` and `"pages": null`.

```bash
sheaf info report.pdf
sheaf info a.pdf b.pdf --json
```

---

### 11. `sheaf verify`
Verify PDF files for structural integrity (loads cleanly, valid catalog & page tree, resolvable page content streams and resources). With `--deep`, also runs `qpdf --check` when available.

```bash
sheaf verify report.pdf
sheaf verify out/*.pdf --deep --json
```

---

### 12. `sheaf capabilities`
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
