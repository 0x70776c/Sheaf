#!/usr/bin/env python3
"""Generate all application icons from a high-resolution source image."""
import os
import sys
import numpy as np
from PIL import Image

def generate(source_path: str):
    root_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
    src = Image.open(source_path).convert("RGBA")
    arr = np.array(src, dtype=np.float32)

    cx, cy = 511.5, 511.5
    half = 383.5
    ys, xs = np.mgrid[0:1024, 0:1024].astype(np.float32)
    nx = np.abs((xs - cx) / half)
    ny = np.abs((ys - cy) / half)
    squircle_dist = nx**5.0 + ny**5.0

    # Smooth anti-aliased edge at squircle boundary
    eps = 0.012
    squircle_weight = np.clip((1.0 + eps - squircle_dist) / (2 * eps), 0.0, 1.0)

    # Drop shadow outside squircle
    min_c = np.min(arr[:, :, :3], axis=2)
    shadow_raw = np.clip((252.0 - min_c) / 252.0, 0.0, 1.0)
    shadow_alpha = np.power(shadow_raw, 0.9) * 1.25
    shadow_alpha = np.clip(shadow_alpha, 0.0, 1.0)

    # Edge fade at borders
    border_dist = np.minimum(np.minimum(xs, 1023 - xs), np.minimum(ys, 1023 - ys))
    border_fade = np.clip((border_dist - 15) / 30.0, 0.0, 1.0)
    shadow_alpha *= border_fade

    out = np.zeros((1024, 1024, 4), dtype=np.float32)
    shadow_rgb = np.array([15.0, 18.0, 25.0])

    for c in range(3):
        out[:, :, c] = squircle_weight * arr[:, :, c] + (1.0 - squircle_weight) * shadow_rgb[c]

    out[:, :, 3] = (squircle_weight + (1.0 - squircle_weight) * shadow_alpha) * 255.0

    out_uint8 = np.clip(out, 0, 255).astype(np.uint8)
    master = Image.fromarray(out_uint8, "RGBA")

    # Target directories
    os.makedirs(os.path.join(root_dir, "assets/icon"), exist_ok=True)
    os.makedirs(os.path.join(root_dir, "docs/images"), exist_ok=True)
    os.makedirs(os.path.join(root_dir, "apps/desktop/public"), exist_ok=True)
    os.makedirs(os.path.join(root_dir, "apps/desktop/src-tauri/icons"), exist_ok=True)

    # Master 1024
    master.save(os.path.join(root_dir, "assets/icon/icon-1024.png"))

    # Iconset for iconutil
    iconset_dir = "/tmp/sheaf.iconset"
    os.makedirs(iconset_dir, exist_ok=True)

    sizes = [
        ("icon_16x16.png", 16),
        ("icon_16x16@2x.png", 32),
        ("icon_32x32.png", 32),
        ("icon_32x32@2x.png", 64),
        ("icon_128x128.png", 128),
        ("icon_128x128@2x.png", 256),
        ("icon_256x256.png", 256),
        ("icon_256x256@2x.png", 512),
        ("icon_512x512.png", 512),
        ("icon_512x512@2x.png", 1024),
    ]

    for name, sz in sizes:
        resized = master.resize((sz, sz), Image.Resampling.LANCZOS)
        resized.save(os.path.join(iconset_dir, name))

    icns_path = os.path.join(root_dir, "apps/desktop/src-tauri/icons/icon.icns")
    os.system(f"iconutil -c icns {iconset_dir} -o {icns_path}")
    os.system(f"rm -rf {iconset_dir}")

    # App icons
    master.resize((512, 512), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "apps/desktop/src-tauri/icons/icon.png"))
    master.resize((128, 128), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "apps/desktop/src-tauri/icons/128x128.png"))
    master.resize((256, 256), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "apps/desktop/src-tauri/icons/128x128@2x.png"))
    master.resize((32, 32), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "apps/desktop/src-tauri/icons/32x32.png"))
    master.resize((256, 256), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "apps/desktop/public/icon.png"))
    master.resize((256, 256), Image.Resampling.LANCZOS).save(os.path.join(root_dir, "docs/images/icon-256.png"))
    print("Icons successfully generated.")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: generate_icons.py <path-to-source-1024.png>")
        sys.exit(1)
    generate(sys.argv[1])
