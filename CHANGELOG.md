# Changelog

All notable changes to Sheaf will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-26

Initial open-source release of Sheaf — a fast, 100% offline PDF toolkit for macOS.

### Features
- **Merge (`sheaf merge`)**: Combine multiple PDF documents preserving page order and bookmarks hierarchy.
- **Split (`sheaf split`)**: Extract page ranges (`--ranges`), split every N pages (`--every`), split by bookmarks (`--bookmarks-level`), or combine extracted ranges (`--combine`).
- **Page Operations (`sheaf pages`)**: Rotate (`rotate`), delete (`delete`), reorder (`reorder`), and extract (`extract`) pages.
- **Render (`sheaf render`)**: High-performance rasterization to PNG using embedded PDFium with parallel worker threads and thumbnail caching.
- **Compress (`sheaf compress`)**: PDF stream compression and image downsampling with presets (`low`, `medium`, `high`) and optional grayscale conversion (`--grayscale`).
- **Convert (`sheaf convert`)**: Bi-directional conversion between PDF and PNG/JPEG images (`to-images`, `from-images`), and PDF and Markdown (`to-markdown`, `from-markdown`).
- **Watermark (`sheaf watermark`)**: Overlay custom text stamps with configurable rotation, placement, opacity, WinAnsi font encoding, and proper clipping isolation.
- **Page Numbering (`sheaf number`)**: Add dynamic page numbers (`{n}`, `{total}`) with custom format, font size, margins, and position.
- **Security & Password Pipeline (`sheaf protect`, `sheaf unlock`)**: 
  - Centralized safe password ingestion via `--password-env`, `--password-file`, and `--password-stdin` using `secrecy::SecretString`.
  - AES-256 (Revision 6) encryption with granular permission flags (`--no-print`, `--no-copy`, `--no-modify`).
  - Decrypt protected documents into unencrypted PDFs (`sheaf unlock`).
- **Metadata Management (`sheaf meta`)**: Inspect (`get`), modify (`set`), and strip (`strip`) standard Info dictionary fields and XMP metadata streams.
- **Document Analysis & Verification (`sheaf info`, `sheaf verify`)**: Comprehensive PDF inspection and structure validation (including optional deep validation via `qpdf`).
- **Machine-Readable CLI Contract**:
  - Global `--json` emitting schema-versioned envelopes (`"schema": 1`) on stdout for all subcommands and errors.
  - Subcommands `capabilities` and `completions` (Bash, Zsh, Fish).
  - Robust panic hook formatting panics as JSON envelopes without secret leakage.
- **Native Desktop Application**:
  - Built with Tauri 2 and Svelte 5 with frosted liquid glass design, draggable overlay titlebar, and responsive dark/light theme.
  - Interactive page thumbnail grid with multi-selection (`⇧+Click`, `⌘+Click`, `⌘A`).
  - Command Palette (`⌘K`) for quick navigation.
  - Side panels for Merge, Split, Pages, Compress, Convert, Watermark, Number, Protect/Unlock, and Metadata.
  - 100% offline: strict CSP, zero analytics, zero network calls.
