//! Lossless WebP of PNG pictures, with the bake's encoder (libwebp at its slowest): the same pixels, smaller. The site
//! ships the fighters' sheets this way (`site/build.sh`): 2.8 MB of PNG pages are 2.0 MB of WebP.
//!
//! ```text
//! cargo run --release -p arena-vfx --example webp -- <in.png> <out.webp> [<in.png> <out.webp> ...]
//! ```

#![allow(clippy::disallowed_methods)]

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || !args.len().is_multiple_of(2) {
        return Err("usage: webp <in.png> <out.webp> [<in.png> <out.webp> ...]".into());
    }
    for pair in args.chunks(2) {
        let (from, to) = (&pair[0], &pair[1]);
        let img = image::open(from)
            .map_err(|e| format!("{from}: {e}"))?
            .to_rgba8();
        let mut config = webp::WebPConfig::new().map_err(|()| "webp: no config")?;
        // exact: keep the colour under a clear pixel as it is, so the picture is the PNG's to the byte
        (config.lossless, config.quality, config.method, config.exact) = (1, 100.0, 6, 1);
        let bytes = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height())
            .encode_advanced(&config)
            .map_err(|e| format!("{from}: {e:?}"))?;
        std::fs::write(to, &*bytes).map_err(|e| format!("{to}: {e}"))?;
        println!(
            "{to}: {} bytes (the PNG {})",
            bytes.len(),
            std::fs::metadata(from).map_or(0, |m| m.len())
        );
    }
    Ok(())
}
