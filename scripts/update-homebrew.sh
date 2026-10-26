#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
    echo "Usage: $0 <version> [path-or-url-to-SHA256SUMS]" >&2
    exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
SHA256_SOURCE="${2:-}"

DMG_FILE="Sheaf-$VERSION-universal.dmg"
CLI_TAR="Sheaf-cli-$VERSION-universal.tar.gz"

TMP_SHA="/tmp/sheaf-update-homebrew-sums"
rm -f "$TMP_SHA"

if [ -n "$SHA256_SOURCE" ]; then
    if [[ "$SHA256_SOURCE" =~ ^https?:// ]]; then
        echo "Downloading SHA256SUMS from $SHA256_SOURCE..."
        curl -sSL "$SHA256_SOURCE" -o "$TMP_SHA"
    else
        cp "$SHA256_SOURCE" "$TMP_SHA"
    fi
elif [ -f "$DIST_DIR/SHA256SUMS" ]; then
    cp "$DIST_DIR/SHA256SUMS" "$TMP_SHA"
else
    RELEASE_URL="https://github.com/0x70776c/Sheaf/releases/download/v${VERSION}/SHA256SUMS"
    echo "Fetching published SHA256SUMS from $RELEASE_URL..."
    if ! curl -sSfL "$RELEASE_URL" -o "$TMP_SHA" 2>/dev/null; then
        echo "Error: Could not retrieve SHA256SUMS. Provide a path or URL, or generate dist/SHA256SUMS." >&2
        exit 1
    fi
fi

DMG_SHA256=$(grep "$DMG_FILE" "$TMP_SHA" | awk '{print $1}')
CLI_SHA256=$(grep "$CLI_TAR" "$TMP_SHA" | awk '{print $1}')
rm -f "$TMP_SHA"

if [ -z "$DMG_SHA256" ] || [ -z "$CLI_SHA256" ]; then
    echo "Error: Could not find checksums for $DMG_FILE and $CLI_TAR in SHA256SUMS" >&2
    exit 1
fi

echo "Updating Homebrew formula: sheaf.rb (version $VERSION, sha256 $CLI_SHA256)"
sed -i '' "s/version \".*\"/version \"$VERSION\"/" "$ROOT_DIR/packaging/homebrew/sheaf.rb"
sed -i '' "s/sha256 \".*\"/sha256 \"$CLI_SHA256\"/" "$ROOT_DIR/packaging/homebrew/sheaf.rb"

echo "Updating Homebrew cask: sheaf.rb (version $VERSION, sha256 $DMG_SHA256)"
sed -i '' "s/version \".*\"/version \"$VERSION\"/" "$ROOT_DIR/packaging/homebrew/Casks/sheaf.rb"
sed -i '' "s/sha256 \".*\"/sha256 \"$DMG_SHA256\"/" "$ROOT_DIR/packaging/homebrew/Casks/sheaf.rb"

echo "Homebrew definitions successfully updated."
