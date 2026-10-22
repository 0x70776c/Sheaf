#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <version>" >&2
    exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

CLI_TAR="$DIST_DIR/Sheaf-cli-$VERSION-universal.tar.gz"
DMG_FILE="$DIST_DIR/Sheaf-$VERSION-universal.dmg"

if [ ! -f "$CLI_TAR" ] || [ ! -f "$DMG_FILE" ]; then
    echo "Error: Release artifacts not found in $DIST_DIR for version $VERSION" >&2
    exit 1
fi

CLI_SHA256=$(shasum -a 256 "$CLI_TAR" | awk '{print $1}')
DMG_SHA256=$(shasum -a 256 "$DMG_FILE" | awk '{print $1}')

echo "Updating Homebrew formula: sheaf.rb (version $VERSION, sha256 $CLI_SHA256)"
sed -i '' "s/version \".*\"/version \"$VERSION\"/" "$ROOT_DIR/packaging/homebrew/sheaf.rb"
sed -i '' "s/sha256 \".*\"/sha256 \"$CLI_SHA256\"/" "$ROOT_DIR/packaging/homebrew/sheaf.rb"

echo "Updating Homebrew cask: sheaf.rb (version $VERSION, sha256 $DMG_SHA256)"
sed -i '' "s/version \".*\"/version \"$VERSION\"/" "$ROOT_DIR/packaging/homebrew/Casks/sheaf.rb"
sed -i '' "s/sha256 \".*\"/sha256 \"$DMG_SHA256\"/" "$ROOT_DIR/packaging/homebrew/Casks/sheaf.rb"

echo "Homebrew definitions successfully updated."
