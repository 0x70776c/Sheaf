# Sheaf Homebrew Formulas & Casks

This directory contains Homebrew formula and cask templates for distributing Sheaf via a custom tap or local install.

## Files
- `sheaf.rb`: Formula for the standalone universal CLI binary (`bin/sheaf`, `lib/libpdfium.dylib`, shell completions).
- `Casks/sheaf.rb`: Cask for the universal macOS desktop application (`Sheaf.app`).

## Release Checksums
The `sha256` fields in these templates use placeholders until an official release is published on GitHub.
Because GitHub Actions compiles and signs artifacts on CI runners, release checksums must be populated from the published release's `SHA256SUMS`.

To update the formulas after a GitHub release:

```bash
# Automatically fetches the published SHA256SUMS from GitHub Releases and updates both definitions:
./scripts/update-homebrew.sh 0.1.0

# Or update using a local SHA256SUMS file:
./scripts/update-homebrew.sh 0.1.0 dist/SHA256SUMS
```
