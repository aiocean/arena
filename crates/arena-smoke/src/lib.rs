//! The smoke test of the live arena site, its plain parts: what a run found (`Summary`), what noise it ignores, how a
//! screenshot is judged (painted, moving), and the review pages it writes. The browser that drives the page is the
//! binary's (`main.rs`); everything here is tested without one.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// One check: its name, whether it passed, and what was seen.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Check {
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

impl Check {
    pub fn new(name: &str, pass: bool, detail: impl Into<String>) -> Check {
        Check {
            name: name.into(),
            pass,
            detail: detail.into(),
        }
    }
}

/// What a run found, written as `summary.json` beside its screenshot.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    /// When the run began, UTC (`stamp`).
    pub at: String,
    pub url: String,
    pub pass: bool,
    pub checks: Vec<Check>,
    /// The page's status line when the run ended.
    pub status_line: String,
    /// Console errors, page errors and failed requests that failed the run.
    pub errors: Vec<String>,
    /// What was seen and let pass as noise, with why.
    pub ignored: Vec<String>,
    /// How long the run took, in seconds.
    pub seconds: f64,
    /// What a first visit downloaded (a fresh browser, nothing cached) and how long it took to see the game.
    #[serde(default)]
    pub weight: Weight,
}

/// A first visit's bytes and timings: every response's bytes on the wire (`encodedDataLength`: compressed, headers
/// included) by kind, the largest files, the seconds from navigation to the game starting and to the first live
/// frame. The relay's WebTransport stream is not an HTTP request and is not counted.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Weight {
    pub bytes: u64,
    pub requests: u32,
    /// Kind (`kind_of`), its bytes and its requests, the heaviest first.
    pub by_kind: Vec<(String, u64, u32)>,
    /// The ten heaviest responses: URL (path only) and bytes.
    pub largest: Vec<(String, u64)>,
    /// Navigation to the status line saying `Seed ...` (the game's wasm fetched, compiled and running).
    pub game_start_s: Option<f64>,
    /// Navigation to the first match frame from the relay (`window.__arena.live.frames > 0`).
    pub first_live_frame_s: Option<f64>,
}

impl Weight {
    /// Sum `responses` (URL, MIME type, bytes on the wire) into a weight.
    pub fn of(responses: &[(String, String, u64)]) -> Weight {
        let mut kinds: HashMap<&'static str, (u64, u32)> = HashMap::new();
        for (url, mime, bytes) in responses {
            let k = kinds.entry(kind_of(url, mime)).or_default();
            k.0 += bytes;
            k.1 += 1;
        }
        let mut by_kind: Vec<_> = kinds
            .into_iter()
            .map(|(k, (b, n))| (k.to_string(), b, n))
            .collect();
        by_kind.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut largest: Vec<_> = responses.iter().map(|(u, _, b)| (path_of(u), *b)).collect();
        largest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        largest.truncate(10);
        Weight {
            bytes: responses.iter().map(|r| r.2).sum(),
            requests: u32::try_from(responses.len()).unwrap_or(u32::MAX),
            by_kind,
            largest,
            ..Weight::default()
        }
    }

    /// One line for the console and the review page.
    pub fn line(&self) -> String {
        let kinds: Vec<String> = self
            .by_kind
            .iter()
            .map(|(k, b, n)| format!("{k} {} ({n})", mb(*b)))
            .collect();
        let secs = |s: Option<f64>| s.map_or("never".to_string(), |s| format!("{s:.1} s"));
        format!(
            "{} in {} requests: {}; game started {}, first live frame {}",
            mb(self.bytes),
            self.requests,
            kinds.join(", "),
            secs(self.game_start_s),
            secs(self.first_live_frame_s)
        )
    }
}

/// Bytes as megabytes, one decimal place below 10 MB, two above none.
pub fn mb(bytes: u64) -> String {
    let m = bytes as f64 / 1_000_000.0;
    if m >= 10.0 {
        format!("{m:.1} MB")
    } else {
        format!("{m:.2} MB")
    }
}

/// What kind of file a response is, by its type and else its extension: wasm, js, image, json, html, css, font,
/// other.
pub fn kind_of(url: &str, mime: &str) -> &'static str {
    let path = path_of(url);
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if mime == "application/wasm" || ext == "wasm" {
        "wasm"
    } else if mime.contains("javascript") || ext == "js" || ext == "mjs" {
        "js"
    } else if mime.starts_with("image/") {
        "image"
    } else if mime.contains("json") || ext == "json" {
        "json"
    } else if mime.starts_with("text/html") {
        "html"
    } else if mime.starts_with("text/css") {
        "css"
    } else if mime.starts_with("font/")
        || mime.contains("font")
        || ["woff", "woff2", "ttf", "otf"].contains(&ext.as_str())
    {
        "font"
    } else {
        "other"
    }
}

/// A URL's path, its query (which may hold a token) and its origin left out for another site's file only.
fn path_of(url: &str) -> String {
    let no_query = url.split(['?', '#']).next().unwrap_or(url);
    match no_query.split_once("://") {
        Some((_, rest)) => match rest.split_once('/') {
            Some((host, path))
                if host.ends_with("arena.aiocean.io")
                    || host.starts_with("127.0.0.1")
                    || host.starts_with("localhost") =>
            {
                format!("/{path}")
            }
            _ => rest.to_string(),
        },
        None => no_query.to_string(),
    }
}

impl Summary {
    pub fn finish(&mut self) {
        self.pass = !self.checks.is_empty() && self.checks.iter().all(|c| c.pass);
    }
}

/// A request the run lets fail, and why: other sites' (the page's fonts) and the icon the browser asks for unasked.
/// Every request to the page's own origin that fails or answers 4xx/5xx fails the run.
pub fn ignored_request(url: &str, origin: &str) -> Option<&'static str> {
    if !url.starts_with(origin) {
        return Some("another origin (fonts, the relay): not the game's own assets");
    }
    if url.trim_end_matches('/').ends_with("/favicon.ico") {
        return Some("the browser's unasked favicon request");
    }
    None
}

/// A console error or page error the run lets pass, and why.
pub fn ignored_console(text: &str) -> Option<&'static str> {
    // winit hands control back to the browser by throwing; site/assets/arena.js catches it
    if text.contains("Using exceptions for control flow") {
        return Some(
            "winit's event loop throws to hand control to the browser (caught by arena.js)",
        );
    }
    // the fonts' stylesheet from Google, blocked or slow: the page falls back to its own fonts
    if text.contains("fonts.googleapis.com") || text.contains("fonts.gstatic.com") {
        return Some("Google Fonts: the page falls back to system fonts");
    }
    if text.contains("favicon.ico") {
        return Some("the browser's unasked favicon request");
    }
    None
}

/// `text` with anything that looks like a token (a run of 32 or more URL-safe characters) replaced: the relay's URL
/// carries its subscribe token in its path, and a run's output is published.
pub fn redact(text: &str) -> String {
    let token_char =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | '%');
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.chars().count() >= 32 && run.chars().any(|c| c.is_ascii_digit()) {
            out.push_str("<redacted>");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if token_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// An RGBA picture.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

impl Picture {
    pub fn from_png(bytes: &[u8]) -> Result<Picture, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let size = reader
            .output_buffer_size()
            .ok_or("the picture is too big")?;
        let mut buf = vec![0; size];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let (w, h) = (info.width as usize, info.height as usize);
        let rgba = match info.color_type {
            png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
            png::ColorType::Rgb => buf[..w * h * 3]
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|&[r, g, b]| [r, g, b, 255])
                .collect(),
            png::ColorType::Grayscale => {
                buf[..w * h].iter().flat_map(|&g| [g, g, g, 255]).collect()
            }
            png::ColorType::GrayscaleAlpha => buf[..w * h * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .flat_map(|&[g, a]| [g, g, g, a])
                .collect(),
            png::ColorType::Indexed => return Err("an indexed picture after expansion".into()),
        };
        Ok(Picture {
            width: w,
            height: h,
            rgba,
        })
    }

    fn px(&self, x: usize, y: usize) -> [u8; 3] {
        let i = (y * self.width + x) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]]
    }
}

/// How painted a picture is: how many colours it has (to 4 bits a channel) and the share of its pixels that are not
/// its most common colour (the background).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Paint {
    pub colours: usize,
    pub off_background: f64,
}

impl Paint {
    /// Painted: more than a flat fill and a few shapes on it.
    pub fn painted(&self) -> bool {
        self.colours >= 24 && self.off_background >= 0.05
    }
}

pub fn paint(p: &Picture) -> Paint {
    let mut counts: HashMap<[u8; 3], usize> = HashMap::new();
    for y in 0..p.height {
        for x in 0..p.width {
            let [r, g, b] = p.px(x, y);
            *counts.entry([r >> 4, g >> 4, b >> 4]).or_default() += 1;
        }
    }
    let total = (p.width * p.height).max(1);
    let background = counts.values().copied().max().unwrap_or(0);
    Paint {
        colours: counts.len(),
        off_background: (total - background) as f64 / total as f64,
    }
}

/// The share of pixels in the fight area (the middle of the picture, the HUD's top strip and the edges left out) that
/// changed between two pictures of one size by more than a little in any channel.
pub fn changed(a: &Picture, b: &Picture) -> Result<f64, String> {
    if a.width != b.width || a.height != b.height {
        return Err(format!(
            "the pictures differ in size: {}x{} and {}x{}",
            a.width, a.height, b.width, b.height
        ));
    }
    let (x0, x1) = (a.width / 10, a.width - a.width / 10);
    let (y0, y1) = (a.height / 5, a.height - a.height / 20);
    let mut moved = 0usize;
    let mut total = 0usize;
    for y in y0..y1 {
        for x in x0..x1 {
            let (p, q) = (a.px(x, y), b.px(x, y));
            total += 1;
            if p.iter().zip(q).any(|(u, v)| u.abs_diff(v) > 24) {
                moved += 1;
            }
        }
    }
    Ok(moved as f64 / total.max(1) as f64)
}

/// A UTC time as `2026-11-18T01-02-03Z`: sortable, and a directory name in a URL as it is.
pub fn stamp(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}-{:02}-{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Days since 1970-01-01 as a date (Howard Hinnant's `civil_from_days`).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Whether a directory name is a run's (`stamp`).
pub fn is_run(name: &str) -> bool {
    name.len() == 20
        && name.ends_with('Z')
        && name.as_bytes()[10] == b'T'
        && name
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | 'T' | 'Z'))
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "body{font:15px/1.5 system-ui,sans-serif;background:#0f1117;color:#e6e6e6;max-width:1100px;margin:2rem auto;padding:0 1rem}
a{color:#8ab4ff}h1{font-size:1.4rem}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #2a2f3a;padding:.4rem .6rem;text-align:left;vertical-align:top}
.pass{color:#5fd38d;font-weight:600}.fail{color:#ff6b6b;font-weight:600}img{max-width:100%;border:1px solid #2a2f3a}code,pre{font-family:ui-monospace,monospace;font-size:13px}
pre{white-space:pre-wrap;background:#171a22;padding:.6rem;border-radius:4px}.muted{color:#9aa3b2}";

fn verdict(pass: bool) -> &'static str {
    if pass {
        "<span class=\"pass\">PASS</span>"
    } else {
        "<span class=\"fail\">FAIL</span>"
    }
}

/// A run's review page: the verdict, each check, what failed it and what was let pass, the screenshot.
pub fn run_page(s: &Summary) -> String {
    let rows: String = s
        .checks
        .iter()
        .map(|c| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
                verdict(c.pass),
                esc(&c.name),
                esc(&c.detail)
            )
        })
        .collect();
    let list = |items: &[String]| {
        if items.is_empty() {
            "<p class=\"muted\">none</p>".to_string()
        } else {
            format!("<pre>{}</pre>", esc(&items.join("\n")))
        }
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>arena smoke {at}</title><style>{STYLE}</style></head><body>
<p><a href=\"../\">all runs</a></p>
<h1>Arena smoke test, {at}: {v}</h1>
<p>{url} · {secs:.1} s · status line: <code>{status}</code></p>
<table><tr><th></th><th>check</th><th>what was seen</th></tr>{rows}</table>
<h2>Errors</h2>{errors}
<h2>Ignored as noise</h2>{ignored}
<h2>The page</h2><p><img src=\"screenshot.png\" alt=\"the page when the run ended\"></p>
<p class=\"muted\"><a href=\"summary.json\">summary.json</a></p>
</body></html>
",
        at = esc(&s.at),
        v = verdict(s.pass),
        url = esc(&s.url),
        secs = s.seconds,
        status = esc(&s.status_line),
        errors = list(&s.errors),
        ignored = list(&s.ignored),
    )
}

/// The index of runs, newest first, each with its checks.
pub fn index_page(runs: &[Summary]) -> String {
    let rows: String = runs
        .iter()
        .map(|s| {
            let checks: String = s
                .checks
                .iter()
                .map(|c| format!("{} {}", verdict(c.pass), esc(&c.name)))
                .collect::<Vec<_>>()
                .join("<br>");
            format!(
                "<tr><td><a href=\"{at}/\">{at}</a></td><td>{}</td><td>{checks}</td><td><a href=\"{at}/\"><img src=\"{at}/screenshot.png\" alt=\"\" width=\"240\" loading=\"lazy\"></a></td></tr>",
                verdict(s.pass),
                at = esc(&s.at),
            )
        })
        .collect();
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>arena smoke runs</title><style>{STYLE}</style></head><body>
<h1>Arena smoke runs</h1>
<p class=\"muted\">The smoke test of https://arena.aiocean.io/ after each deploy (<code>cargo run --release -p arena-smoke -- --review ~/services/arena/site/review/smoke</code>), newest first, the last {KEEP} kept.</p>
<table><tr><th>run (UTC)</th><th></th><th>checks</th><th></th></tr>{rows}</table>
</body></html>
"
    )
}

/// How many runs the review directory keeps.
pub const KEEP: usize = 30;

/// Write a run into `dir/<at>/` (its page, summary and screenshot), drop the runs past the last `KEEP`, and write the
/// index of the rest. Returns the run's directory.
pub fn write_review(
    dir: &Path,
    s: &Summary,
    screenshot: &[u8],
) -> Result<std::path::PathBuf, String> {
    let run = dir.join(&s.at);
    fs::create_dir_all(&run).map_err(|e| format!("{}: {e}", run.display()))?;
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    let write = |name: &str, bytes: &[u8]| {
        let p = run.join(name);
        fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))
    };
    write("summary.json", json.as_bytes())?;
    write("screenshot.png", screenshot)?;
    write("index.html", run_page(s).as_bytes())?;

    let mut names: Vec<String> = fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_run(n))
        .collect();
    names.sort_unstable_by(|a, b| b.cmp(a));
    for old in names.iter().skip(KEEP) {
        let p = dir.join(old);
        fs::remove_dir_all(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let runs: Vec<Summary> = names
        .iter()
        .take(KEEP)
        .filter_map(|n| fs::read_to_string(dir.join(n).join("summary.json")).ok())
        .filter_map(|t| serde_json::from_str(&t).ok())
        .collect();
    let index = dir.join("index.html");
    fs::write(&index, index_page(&runs)).map_err(|e| format!("{}: {e}", index.display()))?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_visit_is_weighed_by_kind_heaviest_first() {
        let r = |u: &str, m: &str, b: u64| (u.to_string(), m.to_string(), b);
        let w = Weight::of(&[
            r("https://arena.aiocean.io/", "text/html", 4_000),
            r(
                "https://arena.aiocean.io/game/arena_game_bg.wasm",
                "application/wasm",
                6_700_000,
            ),
            r(
                "https://arena.aiocean.io/game/arena_game.js",
                "text/javascript",
                30_000,
            ),
            r(
                "https://arena.aiocean.io/game/assets/vfx/vfx-1.webp",
                "image/webp",
                1_200_000,
            ),
            r(
                "https://arena.aiocean.io/game/assets/fighters/a.png",
                "image/png",
                900_000,
            ),
            r(
                "https://arena.aiocean.io/live.json?x=secret",
                "application/json",
                300,
            ),
            r("https://fonts.gstatic.com/s/x.woff2", "font/woff2", 20_000),
        ]);
        assert_eq!(w.bytes, 8_854_300);
        assert_eq!(w.requests, 7);
        assert_eq!(w.by_kind[0], ("wasm".to_string(), 6_700_000, 1));
        assert_eq!(w.by_kind[1], ("image".to_string(), 2_100_000, 2));
        assert_eq!(w.largest[0].0, "/game/arena_game_bg.wasm");
        assert!(
            w.largest.iter().all(|(u, _)| !u.contains("secret")),
            "a query never shown: {:?}",
            w.largest
        );
        assert!(
            w.largest
                .iter()
                .any(|(u, _)| u == "fonts.gstatic.com/s/x.woff2")
        );
        assert!(
            w.line()
                .starts_with("8.85 MB in 7 requests: wasm 6.70 MB (1), image 2.10 MB (2)"),
            "{}",
            w.line()
        );
    }

    #[test]
    fn kinds_by_type_then_extension() {
        assert_eq!(
            kind_of("https://a/x.wasm", "application/octet-stream"),
            "wasm"
        );
        assert_eq!(kind_of("https://a/x", "application/javascript"), "js");
        assert_eq!(kind_of("https://a/x.webp", "image/webp"), "image");
        assert_eq!(
            kind_of("https://a/docs/", "text/html; charset=utf-8"),
            "html"
        );
        assert_eq!(kind_of("https://a/x.woff2", ""), "font");
        assert_eq!(kind_of("https://a/x.bin", ""), "other");
    }

    fn picture(w: usize, h: usize, f: impl Fn(usize, usize) -> [u8; 3]) -> Picture {
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let [r, g, b] = f(x, y);
                rgba.extend([r, g, b, 255]);
            }
        }
        Picture {
            width: w,
            height: h,
            rgba,
        }
    }

    #[test]
    fn a_flat_picture_is_blank_and_a_busy_one_is_painted() {
        assert!(!paint(&picture(64, 64, |_, _| [10, 12, 20])).painted());
        let busy = picture(64, 64, |x, y| {
            [(x * 4) as u8, (y * 4) as u8, ((x + y) * 2) as u8]
        });
        assert!(paint(&busy).painted());
    }

    #[test]
    fn a_moved_shape_is_a_change() -> Result<(), String> {
        let at = |cx: usize| {
            move |x: usize, y: usize| {
                if x.abs_diff(cx) < 6 && y.abs_diff(32) < 10 {
                    [255, 200, 0]
                } else {
                    [0, 0, 0]
                }
            }
        };
        let (a, b) = (picture(100, 64, at(30)), picture(100, 64, at(60)));
        assert_eq!(changed(&a, &a)?, 0.0);
        assert!(changed(&a, &b)? > 0.01);
        assert!(changed(&a, &picture(10, 10, |_, _| [0, 0, 0])).is_err());
        Ok(())
    }

    #[test]
    fn a_png_reads_back() -> Result<(), String> {
        let mut bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut bytes, 2, 1);
            enc.set_color(png::ColorType::Rgb);
            let mut w = enc.write_header().map_err(|e| e.to_string())?;
            w.write_image_data(&[1, 2, 3, 4, 5, 6])
                .map_err(|e| e.to_string())?;
        }
        let p = Picture::from_png(&bytes)?;
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(p.rgba, vec![1, 2, 3, 255, 4, 5, 6, 255]);
        Ok(())
    }

    #[test]
    fn noise_is_named_and_the_games_own_failures_are_not() {
        let o = "https://arena.aiocean.io";
        assert!(ignored_request("https://fonts.gstatic.com/x.woff2", o).is_some());
        assert!(ignored_request("https://arena.aiocean.io/favicon.ico", o).is_some());
        assert!(ignored_request("https://arena.aiocean.io/game/arena_game_bg.wasm", o).is_none());
        assert!(
            ignored_console("Error: Using exceptions for control flow, don't mind me.").is_some()
        );
        assert!(ignored_console("panicked at src/lib.rs").is_none());
    }

    #[test]
    fn tokens_are_redacted() {
        let t = "dial https://relay.example:443/anon/eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOjEyMzQ1Njc4OTB9.abc failed";
        let r = redact(t);
        assert!(!r.contains("eyJ"), "{r}");
        assert!(
            r.starts_with("dial https://relay.example:443/anon/<redacted> failed"),
            "{r}"
        );
        assert_eq!(
            redact("Seed 42 · live · round 2"),
            "Seed 42 · live · round 2"
        );
    }

    #[test]
    fn stamps_sort_as_time_and_name_runs() {
        let t = UNIX_EPOCH + Duration::from_secs(1_795_000_000);
        let s = stamp(t);
        assert_eq!(s, "2026-11-18T11-06-40Z");
        assert!(is_run(&s));
        assert!(!is_run("index.html"));
        assert!(stamp(t + Duration::from_secs(1)) > s);
    }

    #[test]
    fn the_review_keeps_the_last_runs_newest_first() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!("arena-smoke-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for i in 0..(KEEP + 3) {
            let mut s = Summary {
                at: stamp(UNIX_EPOCH + Duration::from_secs(1_795_000_000 + i as u64)),
                checks: vec![Check::new("page", i % 2 == 0, "seen")],
                ..Summary::default()
            };
            s.finish();
            write_review(&dir, &s, b"png")?;
        }
        let runs = fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_dir())
            .count();
        assert_eq!(runs, KEEP);
        let index = fs::read_to_string(dir.join("index.html")).map_err(|e| e.to_string())?;
        let newest = stamp(UNIX_EPOCH + Duration::from_secs(1_795_000_000 + (KEEP + 2) as u64));
        let oldest_kept = stamp(UNIX_EPOCH + Duration::from_secs(1_795_000_003));
        let (a, b) = (index.find(&newest), index.find(&oldest_kept));
        assert!(a.is_some() && b.is_some() && a < b, "newest first");
        assert!(!index.contains(&stamp(UNIX_EPOCH + Duration::from_secs(1_795_000_000))));
        let _ = fs::remove_dir_all(&dir);
        Ok(())
    }
}
