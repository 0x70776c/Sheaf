#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
TMP_BUILD="/tmp/sheaf-universal-build"

PDFIUM_VERSION="8111"
PDFIUM_ARM64_SHA256="46c674298c60123f1195c2a03c65ee08c4783c84874ce4f5b3a054abb3ecaf30"
PDFIUM_X64_SHA256="e7fece4693f65a7e47f84fa38099068d573d4dcbd46b1a3c8d2c26581c308466"

echo "=== Sheaf Universal Build & Packaging ==="
rm -rf "$TMP_BUILD"
mkdir -p "$DIST_DIR" "$TMP_BUILD"

# 1. Universelle PDFium-Bibliothek vorbereiten
echo "[1/7] Preparing Universal PDFium library (v$PDFIUM_VERSION)..."
PDFIUM_DIR="$TMP_BUILD/pdfium"
mkdir -p "$PDFIUM_DIR/arm64" "$PDFIUM_DIR/x64"

ARM64_TAR="$PDFIUM_DIR/pdfium-mac-arm64.tgz"
X64_TAR="$PDFIUM_DIR/pdfium-mac-x64.tgz"

if [ ! -f "$ARM64_TAR" ]; then
    echo "Downloading pdfium-mac-arm64.tgz..."
    curl -sSL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/$PDFIUM_VERSION/pdfium-mac-arm64.tgz" -o "$ARM64_TAR"
fi

if [ ! -f "$X64_TAR" ]; then
    echo "Downloading pdfium-mac-x64.tgz..."
    curl -sSL "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/$PDFIUM_VERSION/pdfium-mac-x64.tgz" -o "$X64_TAR"
fi

echo "Verifying PDFium checksums..."
echo "$PDFIUM_ARM64_SHA256  $ARM64_TAR" | shasum -a 256 -c -
echo "$PDFIUM_X64_SHA256  $X64_TAR" | shasum -a 256 -c -

tar -xzf "$ARM64_TAR" -C "$PDFIUM_DIR/arm64"
tar -xzf "$X64_TAR" -C "$PDFIUM_DIR/x64"

UNIVERSAL_PDFIUM="$TMP_BUILD/libpdfium.dylib"
lipo -create -output "$UNIVERSAL_PDFIUM" "$PDFIUM_DIR/arm64/lib/libpdfium.dylib" "$PDFIUM_DIR/x64/lib/libpdfium.dylib"

PDFIUM_ARCHS=$(lipo -archs "$UNIVERSAL_PDFIUM")
echo "Universal PDFium architectures: $PDFIUM_ARCHS"
if [[ "$PDFIUM_ARCHS" != *"x86_64"* ]] || [[ "$PDFIUM_ARCHS" != *"arm64"* ]]; then
    echo "Error: PDFium is not universal (expected x86_64 arm64, got '$PDFIUM_ARCHS')" >&2
    exit 1
fi

# Update local vendor/pdfium with universal library as well
mkdir -p "$ROOT_DIR/vendor/pdfium/lib"
cp "$UNIVERSAL_PDFIUM" "$ROOT_DIR/vendor/pdfium/lib/libpdfium.dylib"

# 2. CLI universal bauen
echo "[2/7] Building universal CLI..."
cargo build --release --target aarch64-apple-darwin -p sheaf-cli
cargo build --release --target x86_64-apple-darwin -p sheaf-cli

CLI_UNIVERSAL="$DIST_DIR/sheaf"
lipo -create -output "$CLI_UNIVERSAL" \
    "$ROOT_DIR/target/aarch64-apple-darwin/release/sheaf" \
    "$ROOT_DIR/target/x86_64-apple-darwin/release/sheaf"

chmod +x "$CLI_UNIVERSAL"
echo "Ad-hoc signing CLI binary (after stripping extended attributes)..."
xattr -cr "$CLI_UNIVERSAL"
codesign --force --sign - --timestamp=none "$CLI_UNIVERSAL"

# 3. Desktop-App universal bauen
echo "[3/7] Building universal Desktop app..."
npm --prefix "$ROOT_DIR/apps/desktop" run build

(cd "$ROOT_DIR/apps/desktop" && npx tauri build --target universal-apple-darwin --bundles app --no-sign)

APP_SOURCE="$ROOT_DIR/target/universal-apple-darwin/release/bundle/macos/Sheaf.app"
if [ ! -d "$APP_SOURCE" ]; then
    echo "Error: Sheaf.app not found at $APP_SOURCE" >&2
    exit 1
fi

APP_STAGE="$TMP_BUILD/Sheaf.app"
rm -rf "$APP_STAGE"
ditto "$APP_SOURCE" "$APP_STAGE"

echo "Embedding universal PDFium into Sheaf.app/Contents/Frameworks/..."
mkdir -p "$APP_STAGE/Contents/Frameworks"
cp "$UNIVERSAL_PDFIUM" "$APP_STAGE/Contents/Frameworks/libpdfium.dylib"

echo "Ad-hoc signing Sheaf.app (inside-out, stripping extended attributes)..."
xattr -cr "$APP_STAGE"
codesign --force --sign - --timestamp=none "$APP_STAGE/Contents/Frameworks/libpdfium.dylib"
codesign --force --sign - --timestamp=none "$APP_STAGE/Contents/MacOS/sheaf-desktop"
codesign --force --sign - --timestamp=none --deep "$APP_STAGE"

# Also place clean Sheaf.app into dist/
rm -rf "$DIST_DIR/Sheaf.app"
(cd "$TMP_BUILD" && tar -cf - Sheaf.app) | (cd "$DIST_DIR" && tar -xf -)
xattr -cr "$DIST_DIR/Sheaf.app" 2>/dev/null || true

# 4. DMG erzeugen
echo "[4/7] Creating universal DMG..."
DMG_PATH="$DIST_DIR/Sheaf-0.1.0-universal.dmg"
rm -f "$DMG_PATH"

DMG_STAGE="$TMP_BUILD/dmg-stage"
rm -rf "$DMG_STAGE"
mkdir -p "$DMG_STAGE"
ditto "$APP_STAGE" "$DMG_STAGE/Sheaf.app"
ln -s /Applications "$DMG_STAGE/Applications"
xattr -cr "$DMG_STAGE"

hdiutil create -volname "Sheaf" -srcfolder "$DMG_STAGE" -ov -format UDZO "$DMG_PATH"
xattr -cr "$DMG_PATH"
codesign --force --sign - --timestamp=none "$DMG_PATH"

# 5. CLI-Tarball erzeugen (Layout: bin/sheaf, lib/libpdfium.dylib, completions/, docs)
echo "[5/7] Creating CLI tarball..."
TAR_STAGE="$TMP_BUILD/cli-stage"
rm -rf "$TAR_STAGE"
mkdir -p "$TAR_STAGE/bin" "$TAR_STAGE/lib" "$TAR_STAGE/completions"

cp "$CLI_UNIVERSAL" "$TAR_STAGE/bin/sheaf"
# Root symlink to bin/sheaf for direct invocation convenience
(cd "$TAR_STAGE" && ln -sf "bin/sheaf" "sheaf")
cp "$UNIVERSAL_PDFIUM" "$TAR_STAGE/lib/libpdfium.dylib"
cp "$ROOT_DIR/LICENSE-MIT" "$TAR_STAGE/"
cp "$ROOT_DIR/LICENSE-APACHE" "$TAR_STAGE/"
cp "$ROOT_DIR/README.md" "$TAR_STAGE/"

"$CLI_UNIVERSAL" completions zsh > "$TAR_STAGE/completions/_sheaf"
"$CLI_UNIVERSAL" completions bash > "$TAR_STAGE/completions/sheaf.bash"
"$CLI_UNIVERSAL" completions fish > "$TAR_STAGE/completions/sheaf.fish"

TAR_PATH="$DIST_DIR/Sheaf-cli-0.1.0-universal.tar.gz"
tar -czf "$TAR_PATH" -C "$TAR_STAGE" .

# 6. Prüfsummen
echo "[6/7] Generating SHA256SUMS..."
(cd "$DIST_DIR" && shasum -a 256 Sheaf-*.dmg Sheaf-*.tar.gz > SHA256SUMS)

# 7. Verifikations-Schritte
echo "[7/7] Verifying built artifacts..."

echo "Verification 1: lipo -archs dist/sheaf"
V1=$(lipo -archs "$DIST_DIR/sheaf")
echo "  Result: $V1"
[[ "$V1" == *"x86_64"* && "$V1" == *"arm64"* ]]

echo "Verification 2: lipo -archs Sheaf.app/Contents/MacOS/sheaf-desktop"
V2=$(lipo -archs "$APP_STAGE/Contents/MacOS/sheaf-desktop")
echo "  Result: $V2"
[[ "$V2" == *"x86_64"* && "$V2" == *"arm64"* ]]

echo "Verification 3: lipo -archs Sheaf.app/Contents/Frameworks/libpdfium.dylib"
V3=$(lipo -archs "$APP_STAGE/Contents/Frameworks/libpdfium.dylib")
echo "  Result: $V3"
[[ "$V3" == *"x86_64"* && "$V3" == *"arm64"* ]]

echo "Verification 4a: codesign --verify --deep --strict Sheaf.app (staged)"
codesign --verify --deep --strict --verbose=2 "$APP_STAGE"

echo "Verification 4b: codesign --verify --deep --strict Sheaf.app INSIDE mounted DMG"
VERIFY_MOUNT="$TMP_BUILD/dmg-verify-mount"
mkdir -p "$VERIFY_MOUNT"
hdiutil attach "$DMG_PATH" -mountpoint "$VERIFY_MOUNT" -nobrowse -readonly
codesign --verify --deep --strict --verbose=2 "$VERIFY_MOUNT/Sheaf.app"
hdiutil detach "$VERIFY_MOUNT"
rm -rf "$VERIFY_MOUNT"

echo "Verification 5: codesign --verify --strict dist/sheaf"
codesign --verify --strict --verbose=2 "$DIST_DIR/sheaf"

echo "Verification 6: Unpacking tarball and running end-to-end tests (arm64 & x86_64 Rosetta)"
TAR_VERIFY="$TMP_BUILD/tarball-verify"
rm -rf "$TAR_VERIFY"
mkdir -p "$TAR_VERIFY"
tar -xzf "$TAR_PATH" -C "$TAR_VERIFY"

echo "  Verification 6a: arm64 capabilities (env -u PDFIUM_LIB_PATH)"
env -u PDFIUM_LIB_PATH arch -arm64 "$TAR_VERIFY/bin/sheaf" capabilities --json > "$TAR_VERIFY/arm64_caps.json"
grep -q '"ok":true' "$TAR_VERIFY/arm64_caps.json"

echo "  Verification 6b: arm64 render from tarball (env -u PDFIUM_LIB_PATH, --no-cache)"
env -u PDFIUM_LIB_PATH arch -arm64 "$TAR_VERIFY/bin/sheaf" render "$ROOT_DIR/tests/fixtures/fixture_12p.pdf" --page 1 --width 160 --no-cache -o "$TAR_VERIFY/arm64_test.png" --json > "$TAR_VERIFY/arm64_render.json"
grep -q '"ok":true' "$TAR_VERIFY/arm64_render.json"
[ -f "$TAR_VERIFY/arm64_test.png" ]

echo "  Verification 7a: x86_64 Rosetta capabilities (env -u PDFIUM_LIB_PATH)"
env -u PDFIUM_LIB_PATH arch -x86_64 "$TAR_VERIFY/bin/sheaf" capabilities --json > "$TAR_VERIFY/x64_caps.json"
grep -q '"ok":true' "$TAR_VERIFY/x64_caps.json"

echo "  Verification 7b: x86_64 Rosetta render from tarball (env -u PDFIUM_LIB_PATH, --no-cache)"
env -u PDFIUM_LIB_PATH arch -x86_64 "$TAR_VERIFY/bin/sheaf" render "$ROOT_DIR/tests/fixtures/fixture_12p.pdf" --page 1 --width 160 --no-cache -o "$TAR_VERIFY/x64_test.png" --json > "$TAR_VERIFY/x64_render.json"
grep -q '"ok":true' "$TAR_VERIFY/x64_render.json"
[ -f "$TAR_VERIFY/x64_test.png" ]

rm -rf "$TAR_VERIFY"

echo "=== Universal Build and Verification Succeeded! ==="
cat "$DIST_DIR/SHA256SUMS"
