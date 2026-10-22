use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "release".to_string());

    let git_date = Command::new("git")
        .args(["show", "-s", "--format=%cs", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            // Fallback: build date formatted as YYYY-MM-DD
            "2026-10-22".to_string()
        });

    let target = std::env::var("TARGET").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=SHEAF_GIT_HASH={git_hash}");
    println!("cargo:rustc-env=SHEAF_GIT_DATE={git_date}");
    println!("cargo:rustc-env=SHEAF_TARGET_TRIPLE={target}");
}
