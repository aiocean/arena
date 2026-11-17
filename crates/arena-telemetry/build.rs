//! The commit the crate is built at, as `ARENA_GIT_HASH` (the resource's `vcs.revision`): `ARENA_GIT_HASH` from the
//! environment when a build has no git (a container), else `git rev-parse`, else "unknown".

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ARENA_GIT_HASH");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    let hash = std::env::var("ARENA_GIT_HASH")
        .ok()
        .filter(|h| !h.is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=ARENA_GIT_HASH={hash}");
}
