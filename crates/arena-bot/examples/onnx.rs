//! Builds a bot module from an ONNX policy with the `bots/onnx` template, then admits it as a submission would.
//!
//! `cargo run -p arena-bot --example onnx -- MODEL.onnx BOT.wasm`
//!
//! The model takes the observation, float32 `[1, 1889]`, and gives the 27 acts' logits (`docs/bot.md`). Building
//! needs the repo's toolchain, which has the `wasm32-unknown-unknown` target.
//!
//! The same model gives the same bytes from any clone at any path (the house module is pinned by its sha256,
//! `crates/arena-bot/tests/goldens/house_module.sha256`). Two things would tie the bytes to where the clone is:
//! - cargo's `-C metadata` hashes the absolute path of a path dependency outside the template's own workspace (the
//!   template's `arena-mind`, `../../crates/arena-mind`); that hash is the crate's disambiguator, in every mangled
//!   name, and the names order the functions, so the code itself moved. The template is built through a fixed path,
//!   `/tmp/arena-onnx-bot/repo`, a link to this clone, held under a lock while it builds.
//! - source paths in panic locations: the clone's, the registry's and the toolchain's are remapped to fixed prefixes,
//!   with the build's flags set here alone (`CARGO_ENCODED_RUSTFLAGS`), not from the environment or a config file.
//!
//! It is the one way to build the house module, and the profile it is run with does not matter: `cargo run` or
//! `cargo run --release` give the same bytes, as the template is always built `--release` with its own profile, its
//! own target dir and the flags above (the outer cargo's `CARGO_PROFILE_*`, rustflags and target dir are dropped).
//! `site/build.sh` and a re-pin both go through it.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixed place the template is built from: a link to the clone's root, so cargo sees the same paths from any clone.
const FIXED: &str = "/tmp/arena-onnx-bot";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [model, out] = args.as_slice() else {
        return Err("usage: onnx MODEL.onnx BOT.wasm".into());
    };
    let model = std::fs::canonicalize(model).map_err(|e| format!("{model}: {e}"))?;
    let root = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))?;
    let target = root.join("bots/onnx/target");

    std::fs::create_dir_all(FIXED)?;
    // one build through the link at a time, across clones
    let lock = File::create(Path::new(FIXED).join("lock"))?;
    lock.lock()?;
    let link = Path::new(FIXED).join("repo");
    if std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    link_to(&root, &link)?;
    let template = link.join("bots/onnx");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let sysroot = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .current_dir(&root)
        .args(["--print", "sysroot"])
        .output()?;
    let sysroot = String::from_utf8(sysroot.stdout)?.trim().to_string();
    let cargo_home = std::env::var("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".cargo")))?;
    let flags = [
        format!("--remap-path-prefix={}=/arena", link.display()),
        format!("--remap-path-prefix={}=/cargo", cargo_home.display()),
        format!("--remap-path-prefix={sysroot}/lib/rustlib/src/rust=/rust"),
    ];

    let mut build = Command::new(cargo);
    build
        .current_dir(&template)
        .args(["build", "--release", "--target", "wasm32-unknown-unknown"])
        .arg("--manifest-path")
        .arg(template.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(&target)
        .env("ARENA_ONNX", &model)
        .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"));
    // the outer cargo's settings are for the host, not the module; the module's are the template's and the flags above
    for (key, _) in std::env::vars_os() {
        let Some(key) = key.to_str() else { continue };
        let rustflags = key == "RUSTFLAGS"
            || key == "CARGO_BUILD_RUSTFLAGS"
            || (key.starts_with("CARGO_TARGET_") && key.ends_with("_RUSTFLAGS"));
        if rustflags
            || key == "CARGO_TARGET_DIR"
            || key == "CARGO_BUILD_TARGET_DIR"
            || key.starts_with("CARGO_PROFILE_")
        {
            build.env_remove(key);
        }
    }
    let status = build.status()?;
    if !status.success() {
        return Err("building the bot failed".into());
    }
    let built = target.join("wasm32-unknown-unknown/release/onnx_bot.wasm");
    let bytes = std::fs::read(&built)?;
    drop(lock);
    std::fs::write(out, &bytes)?;
    match arena_bot::admit(&bytes) {
        Ok(a) => println!("{out}: {} bytes, admitted, hash {}", bytes.len(), a.hash),
        Err(e) => return Err(format!("{out} is built but would be refused: {e}").into()),
    }
    println!("next: cargo run --release -p arena-bot --features host --example check -- {out}");
    Ok(())
}

#[cfg(unix)]
fn link_to(root: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(root, link)
}

#[cfg(windows)]
fn link_to(root: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(root, link)
}
