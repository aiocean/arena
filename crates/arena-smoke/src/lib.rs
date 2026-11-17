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

/// The machine's load averages over the last 1, 5 and 15 minutes.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Load {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
}

impl Load {
    /// The first three numbers of `/proc/loadavg` (`0.52 0.58 0.59 1/389 12345`) or of macOS's `sysctl -n
    /// vm.loadavg` (`{ 11.46 17.68 17.08 }`).
    pub fn parse(text: &str) -> Option<Load> {
        let mut n = text
            .split(|c: char| c.is_whitespace() || c == '{' || c == '}')
            .filter(|w| !w.is_empty())
            .map(|w| w.replace(',', ".").parse::<f64>());
        Some(Load {
            one: n.next()?.ok()?,
            five: n.next()?.ok()?,
            fifteen: n.next()?.ok()?,
        })
    }

    /// The machine's load now: `/proc/loadavg` on Linux, `sysctl -n vm.loadavg` on macOS; `None` when neither answers.
    pub fn now() -> Option<Load> {
        if let Ok(t) = fs::read_to_string("/proc/loadavg") {
            return Load::parse(&t);
        }
        let out = std::process::Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .output()
            .ok()?;
        Load::parse(&String::from_utf8_lossy(&out.stdout))
    }

    /// `11.46 / 17.68 / 17.08`.
    pub fn shown(&self) -> String {
        format!("{:.2} / {:.2} / {:.2}", self.one, self.five, self.fifteen)
    }
}

/// What a run found, written as `summary.json` beside its screenshot. A run from before a field was added reads
/// back with it empty.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
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
    /// How long the run took, in seconds (both attempts when it was retried).
    pub seconds: f64,
    /// What a first visit downloaded (a fresh browser, nothing cached) and how long it took to see the game.
    #[serde(default)]
    pub weight: Weight,
    /// Which attempt this is: 1, or 2 when the first was retried.
    pub attempt: u32,
    /// The game did not start (its status line never said `Seed`) within the wait: the one failure that is retried.
    pub start_timed_out: bool,
    /// Seconds from the navigation to the game starting (wasm running, the status line set); `None` if it did not.
    pub game_start_seconds: Option<f64>,
    /// Seconds from the navigation to the first live match frame (`window.__arena.live.frames` > 0); `None` if none
    /// came while the run watched.
    pub first_frame_seconds: Option<f64>,
    /// The machine's load averages when the attempt began.
    pub load: Option<Load>,
    /// The deployed commit, when the page or `live.json` names one: the page's build (`window.__arena.build`).
    pub deployed_commit: Option<String>,
    /// Whether the page was built from a tree with uncommitted changes.
    pub deployed_dirty: bool,
    /// The live server's build: the commit on its newest `match.start` in the telemetry store, when it could be read.
    pub server_build: Option<String>,
    /// The page's rules build (`window.__arena.rules`) and the one the server's matches are fought on
    /// (`window.__arena.live.rules`, from its keyframe), when the page says.
    pub page_rules: Option<u32>,
    pub server_rules: Option<u32>,
    /// A warning that does not fail the run: the page and the server built from different commits on one rules build.
    pub build_warning: Option<String>,
    /// The device the run played (`Profile`): its name, empty for the default desktop.
    pub profile: String,
    /// The page's frame rate over ten seconds of the match, as the browser paints it (`requestAnimationFrame`).
    pub fps: Option<Fps>,
    /// Why the run was retried, when it was.
    pub retry_reason: Option<String>,
    /// The first attempt, when the run was retried (this summary is the second's).
    pub first_attempt: Option<Box<Summary>>,
}

/// A first visit's bytes: every response's bytes on the wire (`encodedDataLength`: compressed, headers included) by
/// kind, the largest files and when each finished, and when the last did (the game's start and first live frame are
/// the summary's `game_start_seconds` and `first_frame_seconds`). The relay's WebTransport stream is not an HTTP
/// request and is not counted.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Weight {
    pub bytes: u64,
    pub requests: u32,
    /// Kind (`kind_of`), its bytes and its requests, the heaviest first.
    pub by_kind: Vec<(String, u64, u32)>,
    /// The ten heaviest responses: URL (path only), bytes, and seconds from the first request to its last byte.
    pub largest: Vec<(String, u64, f64)>,
    /// The first request to the last response finished (the pictures arrive after the game has started).
    #[serde(default)]
    pub all_loaded_s: Option<f64>,
    /// The first request to the last fighter sheet's last byte: when the fighters can be drawn.
    #[serde(default)]
    pub fighters_in_s: Option<f64>,
}

impl Weight {
    /// Sum `responses` (URL, MIME type, bytes on the wire, seconds from the first request to its last byte) into a
    /// weight.
    pub fn of(responses: &[(String, String, u64, f64)]) -> Weight {
        let mut kinds: HashMap<&'static str, (u64, u32)> = HashMap::new();
        for (url, mime, bytes, _) in responses {
            let k = kinds.entry(kind_of(url, mime)).or_default();
            k.0 += bytes;
            k.1 += 1;
        }
        let mut by_kind: Vec<_> = kinds
            .into_iter()
            .map(|(k, (b, n))| (k.to_string(), b, n))
            .collect();
        by_kind.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut largest: Vec<_> = responses
            .iter()
            .map(|(u, _, b, t)| (path_of(u), *b, *t))
            .collect();
        largest.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        largest.truncate(10);
        let fighters_in_s = responses
            .iter()
            .filter(|r| r.0.contains("/fighters/"))
            .map(|r| r.3)
            .reduce(f64::max);
        Weight {
            fighters_in_s,
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
        format!(
            "{} in {} requests: {}; fighters in {}, all loaded {}",
            mb(self.bytes),
            self.requests,
            kinds.join(", "),
            self.fighters_in_s
                .map_or("never".to_string(), |s| format!("{s:.1} s")),
            self.all_loaded_s
                .map_or("never".to_string(), |s| format!("{s:.1} s"))
        )
    }
}

/// A device to play the page as: its viewport in CSS pixels, its pixel ratio, a phone's touch and mobile layout, its
/// CPU as a slowdown of this machine's, and its connection (megabits a second down, round trip in ms).
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
    pub mobile: bool,
    pub cpu: f64,
    pub mbps: Option<f64>,
    pub rtt_ms: f64,
}

impl Profile {
    /// The default: a desktop window, this machine's CPU and connection.
    pub const DESKTOP: Profile = Profile {
        name: "",
        width: 1280,
        height: 800,
        dpr: 1.0,
        mobile: false,
        cpu: 1.0,
        mbps: None,
        rtt_ms: 40.0,
    };

    /// `phone`: a portrait phone (390x844, pixel ratio 3, touch), a CPU four times slower, 10 Mbit/s at 150 ms.
    /// `laptop`: a low-end laptop (1366x768), a CPU twice slower, 20 Mbit/s at 40 ms.
    pub fn named(name: &str) -> Option<Profile> {
        match name {
            "phone" => Some(Profile {
                name: "phone",
                width: 390,
                height: 844,
                dpr: 3.0,
                mobile: true,
                cpu: 4.0,
                mbps: Some(10.0),
                rtt_ms: 150.0,
            }),
            "laptop" => Some(Profile {
                name: "laptop",
                width: 1366,
                height: 768,
                dpr: 1.0,
                mobile: false,
                cpu: 2.0,
                mbps: Some(20.0),
                rtt_ms: 40.0,
            }),
            "desktop" => Some(Profile::DESKTOP),
            _ => None,
        }
    }
}

/// A frame rate measured from the gaps between painted frames (milliseconds).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Fps {
    pub frames: u32,
    pub seconds: f64,
    pub fps: f64,
    /// The median gap, the 95th percentile and the longest, ms.
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub longest_ms: f64,
}

impl Fps {
    /// From the gaps between frames; `None` without any.
    pub fn of(gaps: &[f64]) -> Option<Fps> {
        if gaps.is_empty() {
            return None;
        }
        let mut sorted = gaps.to_vec();
        sorted.sort_by(f64::total_cmp);
        let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
        let seconds = gaps.iter().sum::<f64>() / 1000.0;
        Some(Fps {
            frames: u32::try_from(gaps.len()).unwrap_or(u32::MAX),
            seconds,
            fps: gaps.len() as f64 / seconds.max(1e-9),
            p50_ms: at(0.5),
            p95_ms: at(0.95),
            longest_ms: sorted[sorted.len() - 1],
        })
    }

    /// `57.3 fps (median 16.7 ms, p95 25.0 ms, longest 120 ms)`.
    pub fn line(&self) -> String {
        format!(
            "{:.1} fps over {:.0} s (median {:.1} ms, p95 {:.1} ms, longest {:.0} ms)",
            self.fps, self.seconds, self.p50_ms, self.p95_ms, self.longest_ms
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

    /// Whether this attempt is retried, and why: only when the game did not start within the wait. Any other failed
    /// check is the result.
    pub fn retry(&self) -> Option<String> {
        self.start_timed_out.then(|| {
            let page = self
                .checks
                .iter()
                .find(|c| c.name == "page")
                .map_or(String::new(), |c| format!(": {}", c.detail));
            format!(
                "attempt {} timed out waiting for the game to start{page}",
                self.attempt
            )
        })
    }

    /// The run of two attempts: the second's result, with the first kept beside it and why it was retried.
    pub fn retried(first: Summary, mut second: Summary) -> Summary {
        second.attempt = first.attempt + 1;
        second.retry_reason = first.retry();
        second.first_attempt = Some(Box::new(first));
        second
    }

    /// The check named `name`: `PASS`, `FAIL`, or `none` when the run has no such check.
    pub fn check(&self, name: &str) -> &'static str {
        match self.checks.iter().find(|c| c.name == name) {
            Some(c) if c.pass => "PASS",
            Some(_) => "FAIL",
            None => "none",
        }
    }

    /// One line of its timings: `game 5.3 s · first frame 7.8 s · load 2.10 / 1.95 / 1.80 · attempt 1`.
    pub fn timings(&self) -> String {
        let secs = |s: Option<f64>| s.map_or("none".to_string(), |s| format!("{s:.1} s"));
        format!(
            "game start {} · first frame {} · load {} · attempt {}",
            secs(self.game_start_seconds),
            secs(self.first_frame_seconds),
            self.load.map_or("unknown".to_string(), |l| l.shown()),
            self.attempt.max(1)
        )
    }
}

/// Whether two builds name one commit: the same hex, either one written longer (`1a2b3c4` and `1a2b3c4d5e6f`).
pub fn same_commit(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim().to_ascii_lowercase(), b.trim().to_ascii_lowercase());
    !a.is_empty() && !b.is_empty() && a != "unknown" && (a.starts_with(&b) || b.starts_with(&a))
}

/// The page's build against the server's: the `build` check and, when it passes with a doubt, a warning.
///
/// The page and the server must fight on one rules build (the page replays the server's decisions on its own copy of
/// the rules). Built from different commits on the same `RULES_VERSION` is fine (a docs-only site deploy leaves the
/// server older): a warning. Different rules builds fail: the deploy did not ship the page and the server together.
pub fn compare_builds(
    page: Option<&str>,
    server: Option<&str>,
    page_rules: Option<u32>,
    server_rules: Option<u32>,
) -> (Check, Option<String>) {
    let shown = |b: Option<&str>| b.unwrap_or("unknown").to_string();
    let rules = |r: Option<u32>| r.map_or("?".to_string(), |r| r.to_string());
    let detail = format!(
        "page {} (rules {}), server {} (rules {})",
        shown(page),
        rules(page_rules),
        shown(server),
        rules(server_rules)
    );
    if let (Some(p), Some(s)) = (page_rules, server_rules)
        && p != s
    {
        return (
            Check::new(
                "build",
                false,
                format!("{detail}: other rules builds, ship the page and arena-server together"),
            ),
            None,
        );
    }
    let warning = match (page, server) {
        (Some(p), Some(s)) if same_commit(p, s) => None,
        (Some(_), Some(_)) => Some(format!(
            "the page and the server are built from different commits on one rules build ({detail})"
        )),
        _ => Some(format!("a build is not known ({detail})")),
    };
    (Check::new("build", true, detail), warning)
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
<p>{timings} · deployed commit: {commit} · server build: {server}</p>
{retry}
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
        timings = esc(&s.timings()),
        commit = s.deployed_commit.as_deref().map_or("<span class=\"muted\">none named</span>".to_string(), |c| format!("<code>{}</code>", esc(c))),
        server = s.server_build.as_deref().map_or("<span class=\"muted\">unknown</span>".to_string(), |c| format!("<code>{}</code>", esc(c))),
        retry = match (&s.retry_reason, &s.first_attempt) {
            (Some(why), Some(first)) => format!(
                "<p class=\"fail\">Retried: {}</p><p class=\"muted\">The first attempt: {} · {} · its checks: {} · <a href=\"attempt-1.png\">its screenshot</a></p>",
                esc(why),
                if first.pass { "PASS" } else { "FAIL" },
                esc(&first.timings()),
                esc(&first.checks.iter().map(|c| format!("{} {}", c.name, if c.pass { "PASS" } else { "FAIL" })).collect::<Vec<_>>().join(", ")),
            ),
            _ => String::new(),
        },
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
            let secs = |s: Option<f64>| s.map_or("–".to_string(), |s| format!("{s:.1} s"));
            let timing = format!(
                "start {}<br>frame {}<br>load {}<br>attempt {}{}",
                secs(s.game_start_seconds),
                secs(s.first_frame_seconds),
                s.load.map_or("–".to_string(), |l| l.shown()),
                s.attempt.max(1),
                if s.retry_reason.is_some() { " (retried: the game did not start)" } else { "" },
            );
            format!(
                "<tr><td><a href=\"{at}/\">{at}</a></td><td>{}</td><td>{checks}</td><td class=\"muted\">{timing}</td><td><a href=\"{at}/\"><img src=\"{at}/screenshot.png\" alt=\"\" width=\"240\" loading=\"lazy\"></a></td></tr>",
                verdict(s.pass),
                at = esc(&s.at),
            )
        })
        .collect();
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>arena smoke runs</title><style>{STYLE}</style></head><body>
<h1>Arena smoke runs</h1>
<p class=\"muted\">The smoke test of https://arena.aiocean.io/ after each deploy (<code>cargo run --release -p arena-smoke -- --review ~/services/arena/site/review/smoke --telemetry-otlp</code>), newest first, the last {KEEP} kept. Times are from the navigation; load is the machine's 1 / 5 / 15-minute load average when the run began.</p>
<table><tr><th>run (UTC)</th><th></th><th>checks</th><th>timings</th><th></th></tr>{rows}</table>
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
    first_screenshot: Option<&[u8]>,
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
    if let Some(first) = first_screenshot {
        write("attempt-1.png", first)?;
    }
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
    fn frame_rate_from_the_gaps() -> Result<(), String> {
        assert_eq!(Fps::of(&[]), None);
        let mut gaps = vec![16.0; 90];
        gaps.extend([50.0; 9]);
        gaps.push(200.0);
        let f = Fps::of(&gaps).ok_or("no fps")?;
        assert_eq!(f.frames, 100);
        assert!((f.seconds - 2.09).abs() < 1e-9, "{f:?}");
        assert!((f.fps - 100.0 / 2.09).abs() < 1e-9, "{f:?}");
        assert_eq!((f.p50_ms, f.p95_ms, f.longest_ms), (16.0, 50.0, 200.0));
        assert!(
            f.line().starts_with("47.8 fps over 2 s (median 16.0 ms"),
            "{}",
            f.line()
        );
        Ok(())
    }

    #[test]
    fn profiles_by_name() {
        let phone = Profile::named("phone");
        assert_eq!(
            phone.as_ref().map(|p| (p.width, p.height, p.cpu, p.mobile)),
            Some((390, 844, 4.0, true))
        );
        assert_eq!(
            Profile::named("laptop").map(|p| (p.width, p.cpu)),
            Some((1366, 2.0))
        );
        assert_eq!(Profile::named("desktop"), Some(Profile::DESKTOP));
        assert_eq!(Profile::named("watch"), None);
    }

    #[test]
    fn a_visit_is_weighed_by_kind_heaviest_first() {
        let r = |u: &str, m: &str, b: u64| (u.to_string(), m.to_string(), b, 1.0);
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
            w.largest.iter().all(|(u, _, _)| !u.contains("secret")),
            "a query never shown: {:?}",
            w.largest
        );
        assert!(
            w.largest
                .iter()
                .any(|(u, _, _)| u == "fonts.gstatic.com/s/x.woff2")
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
    fn load_reads_linux_and_macos() {
        let l = Load {
            one: 0.52,
            five: 0.58,
            fifteen: 0.59,
        };
        assert_eq!(Load::parse("0.52 0.58 0.59 1/389 12345\n"), Some(l));
        assert_eq!(Load::parse("{ 0.52 0.58 0.59 }\n"), Some(l));
        assert_eq!(Load::parse("{ 0,52 0,58 0,59 }"), Some(l));
        assert_eq!(Load::parse(""), None);
        assert_eq!(Load::parse("{ 1.0 2.0 }"), None);
        assert_eq!(l.shown(), "0.52 / 0.58 / 0.59");
    }

    #[test]
    fn only_a_start_timeout_is_retried_and_the_second_attempt_decides() {
        let attempt = |timed_out: bool, pass: bool| {
            let mut s = Summary {
                attempt: 1,
                start_timed_out: timed_out,
                game_start_seconds: (!timed_out).then_some(4.0),
                checks: vec![
                    Check::new("page", !timed_out, "game not started after 30 s"),
                    Check::new("hud", pass, "seen"),
                ],
                ..Summary::default()
            };
            s.finish();
            s
        };
        // another check failing is the result, not retried
        assert_eq!(attempt(false, false).retry(), None);
        let first = attempt(true, false);
        let why = first.retry();
        assert!(
            why.as_deref()
                .is_some_and(|w| w.contains("timed out") && w.contains("30 s")),
            "{why:?}"
        );
        // the second passes: the run passes, retried, the first kept
        let run = Summary::retried(first.clone(), attempt(false, true));
        assert!(run.pass);
        assert_eq!(run.attempt, 2);
        assert_eq!(run.retry_reason, why);
        assert!(run.first_attempt.as_ref().is_some_and(|f| !f.pass));
        assert!(run.timings().contains("attempt 2"));
        assert_eq!(run.check("page"), "PASS");
        assert_eq!(run.check("live"), "none");
        // both fail: the run fails
        let run = Summary::retried(first, attempt(true, false));
        assert!(!run.pass);
        let page = run_page(&run);
        assert!(page.contains("Retried: attempt 1 timed out"), "{page}");
        assert!(index_page(&[run]).contains("attempt 2 (retried"));
    }

    #[test]
    fn builds_fail_only_on_other_rules() {
        assert!(same_commit("1a2b3c4", "1a2b3c4d5e6f"));
        assert!(!same_commit("1a2b3c4", "9a2b3c4d5e6f"));
        assert!(!same_commit("unknown", "unknown"));
        // one commit: pass, nothing to say
        let (c, w) = compare_builds(Some("1a2b3c4d5e6f"), Some("1a2b3c4d5e6f"), Some(9), Some(9));
        assert!(c.pass && w.is_none(), "{c:?} {w:?}");
        // other commits, one rules build (a docs-only deploy): pass with a warning
        let (c, w) = compare_builds(Some("1a2b3c4d5e6f"), Some("0000000aaaaa"), Some(9), Some(9));
        assert!(c.pass && w.is_some(), "{c:?} {w:?}");
        // other rules builds: fail
        let (c, _) = compare_builds(
            Some("1a2b3c4d5e6f"),
            Some("0000000aaaaa"),
            Some(10),
            Some(9),
        );
        assert!(!c.pass, "{c:?}");
        let (c, _) = compare_builds(Some("1a2b3c4d5e6f"), None, Some(10), Some(9));
        assert!(!c.pass, "{c:?}");
        // the server's build unknown (no telemetry store): pass with a warning
        let (c, w) = compare_builds(Some("1a2b3c4d5e6f"), None, Some(9), None);
        assert!(c.pass && w.is_some(), "{c:?} {w:?}");
    }

    #[test]
    fn an_older_summary_reads_back() -> Result<(), String> {
        let old = r#"{"at":"2026-11-17T21-10-42Z","url":"u","pass":true,"checks":[],"status_line":"","errors":[],"ignored":[],"seconds":1.0}"#;
        let s: Summary = serde_json::from_str(old).map_err(|e| e.to_string())?;
        assert_eq!((s.attempt, s.game_start_seconds, s.load), (0, None, None));
        assert!(s.timings().contains("attempt 1"));
        Ok(())
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
            write_review(&dir, &s, b"png", None)?;
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
