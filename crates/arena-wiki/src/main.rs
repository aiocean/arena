//! `cargo run -p arena-wiki -- <out>`: the wiki, generated from the repository this crate sits in, written into
//! `<out>` (`site/build.sh` writes `dist/wiki`).

use std::path::Path;

fn main() -> std::process::ExitCode {
    let Some(out) = std::env::args().nth(1) else {
        eprintln!("usage: arena-wiki <out>");
        return std::process::ExitCode::from(2);
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    match arena_wiki::build(&root).and_then(|w| {
        w.write(Path::new(&out))?;
        Ok(w.files.len())
    }) {
        Ok(n) => {
            println!("{out}: {n} files");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("arena-wiki: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
