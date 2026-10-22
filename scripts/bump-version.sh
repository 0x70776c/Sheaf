#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <new-version>" >&2
    exit 1
fi

NEW_VERSION="$1"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "Bumping version to $NEW_VERSION..."

# 1. Update Workspace Cargo.toml
python3 -c "
import re
path = '$ROOT_DIR/Cargo.toml'
with open(path, 'r') as f:
    content = f.read()
new_content = re.sub(r'(?m)^version\s*=\s*\"[^\"]+\"', f'version = \"$NEW_VERSION\"', content, count=1)
with open(path, 'w') as f:
    f.write(new_content)
"

# 2. Update apps/desktop/src-tauri/tauri.conf.json
python3 -c "
import json
path = '$ROOT_DIR/apps/desktop/src-tauri/tauri.conf.json'
with open(path, 'r') as f:
    data = json.load(f)
data['version'] = '$NEW_VERSION'
with open(path, 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
"

# 3. Update apps/desktop/package.json
python3 -c "
import json
path = '$ROOT_DIR/apps/desktop/package.json'
with open(path, 'r') as f:
    data = json.load(f)
data['version'] = '$NEW_VERSION'
with open(path, 'w') as f:
    json.dump(data, f, indent=2)
    f.write('\n')
"

# 4. Update lockfiles
echo "Updating Cargo.lock..."
cargo check --workspace

echo "Updating package-lock.json..."
(cd "$ROOT_DIR/apps/desktop" && npm install --package-lock-only)

echo "Version bump to $NEW_VERSION completed."
