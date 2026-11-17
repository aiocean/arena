//! `arena-smoke`: after a deploy, a headless Chrome loads the live arena site and checks that it works.
//!
//! ```text
//! cargo run --release -p arena-smoke -- [--url https://arena.aiocean.io/] [--out target/arena-smoke]
//!     [--review ~/services/arena/site/review/smoke] [--chrome <path>] [--wait 20] [--headful]
//! ```
//!
//! The checks, each PASS or FAIL:
//! - `page`: the page answers 200, the game's wasm loads and the game starts (its status line says `Seed ...`).
//! - `errors`: no console error, no uncaught exception, no failed request for the page's own assets (what is let pass
//!   as noise is `arena_smoke::ignored_request` and `ignored_console`, and is listed in the run's summary).
//! - `live.json`: the site names a relay (200, a `url`).
//! - `live`: the page reached the relay over WebTransport (`window.__arena.live.connected`) and match frames came
//!   (`frames`), and it is watching the server's match, not fighting on its own (the status line's `live`).
//! - `hud`: the canvas is painted (many colours, most pixels not the background).
//! - `weight` (never fails the run): what the first visit downloaded, bytes on the wire by kind and the heaviest
//!   files, and the seconds to the game starting and to the first live frame (`arena_smoke::Weight`).
//! - `advances`: the match moves: the frames counter grows over a few seconds and two screenshots of the fight differ.
//!
//! It writes `screenshot.png` and `summary.json` to `--out`; with `--review <dir>` also a run page into
//! `<dir>/<UTC time>/` and the index of the last 30 runs at `<dir>/index.html`. Exit code 0 when every check passes, 1
//! when one fails, 2 when the browser could not run at all.
//!
//! Chrome: `--chrome`, else `$CHROME`, else a system Chrome or Chromium, else Chrome for Testing downloaded once into
//! `~/.cache/arena-smoke`.

use arena_smoke::{
    Check, Picture, Summary, Weight, changed, ignored_console, ignored_request, paint, redact,
    stamp,
};
use chromiumoxide::cdp::browser_protocol::network::{
    EventLoadingFailed, EventLoadingFinished, EventRequestWillBeSent, EventResponseReceived,
};
use chromiumoxide::cdp::browser_protocol::page::{CaptureScreenshotFormat, Viewport};
use chromiumoxide::cdp::js_protocol::runtime::{
    ConsoleApiCalledType, EventConsoleApiCalled, EventExceptionThrown,
};
use chromiumoxide::fetcher::{BrowserFetcher, BrowserFetcherOptions, BrowserKind};
use chromiumoxide::handler::viewport::Viewport as Window;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

struct Args {
    url: String,
    out: PathBuf,
    review: Option<PathBuf>,
    chrome: Option<PathBuf>,
    wait: u64,
    headful: bool,
}

fn args() -> Result<Args, String> {
    let mut a = Args {
        url: "https://arena.aiocean.io/".into(),
        out: PathBuf::from("target/arena-smoke"),
        review: None,
        chrome: std::env::var_os("CHROME").map(PathBuf::from),
        wait: 20,
        headful: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut value = || it.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--url" => a.url = value()?,
            "--out" => a.out = PathBuf::from(value()?),
            "--review" => a.review = Some(PathBuf::from(value()?)),
            "--chrome" => a.chrome = Some(PathBuf::from(value()?)),
            "--wait" => a.wait = value()?.parse().map_err(|e| format!("--wait: {e}"))?,
            "--headful" => a.headful = true,
            "-h" | "--help" => {
                return Err("usage: arena-smoke [--url URL] [--out DIR] [--review DIR] [--chrome PATH] [--wait SECONDS] [--headful]".into());
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    // a bare host is its root page
    if !a
        .url
        .split_once("://")
        .map_or(&a.url[..], |(_, r)| r)
        .contains('/')
    {
        a.url.push('/');
    }
    Ok(a)
}

/// The origin of a URL (`https://host[:port]`), what counts as the page's own.
fn origin(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split('/').next().unwrap_or(rest);
    let scheme = url.split_once("://").map_or("https", |(s, _)| s);
    format!("{scheme}://{host}")
}

#[tokio::main]
async fn main() -> ExitCode {
    let a = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let at = stamp(SystemTime::now());
    let begun = Instant::now();
    let (mut summary, shot) = match smoke(&a).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("arena-smoke could not run: {}", redact(&e));
            return ExitCode::from(2);
        }
    };
    summary.at = at;
    summary.url = a.url.clone();
    summary.seconds = begun.elapsed().as_secs_f64();
    summary.finish();

    for c in &summary.checks {
        println!(
            "{} {:<10} {}",
            if c.pass { "PASS" } else { "FAIL" },
            c.name,
            c.detail
        );
    }
    for e in &summary.errors {
        println!("  error: {e}");
    }
    for i in &summary.ignored {
        println!("  ignored: {i}");
    }
    let json = serde_json::to_string_pretty(&summary).unwrap_or_default();
    let write = || -> Result<(), String> {
        std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
        std::fs::write(a.out.join("screenshot.png"), &shot).map_err(|e| e.to_string())?;
        std::fs::write(a.out.join("summary.json"), &json).map_err(|e| e.to_string())?;
        Ok(())
    };
    if let Err(e) = write() {
        eprintln!("writing the results: {e}");
    } else {
        println!("screenshot and summary: {}", a.out.display());
    }
    if let Some(dir) = &a.review {
        match arena_smoke::write_review(dir, &summary, &shot) {
            Ok(run) => println!("review: {}", run.display()),
            Err(e) => eprintln!("writing the review: {e}"),
        }
    }
    println!("{}", if summary.pass { "PASS" } else { "FAIL" });
    if summary.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// Where Chrome is: asked for, on the system, or Chrome for Testing fetched once.
async fn chrome(asked: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(p) = asked {
        return Ok(p.to_path_buf());
    }
    let system = [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
    ];
    if let Some(p) = system.iter().map(PathBuf::from).find(|p| p.exists()) {
        return Ok(p);
    }
    let home = std::env::var_os("HOME").ok_or("no HOME for the browser cache")?;
    let cache = PathBuf::from(home).join(".cache/arena-smoke");
    std::fs::create_dir_all(&cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let options = BrowserFetcherOptions::builder()
        .with_kind(BrowserKind::Chrome)
        .with_path(&cache)
        .build()
        .map_err(|e| e.to_string())?;
    let got = BrowserFetcher::new(options)
        .fetch()
        .await
        .map_err(|e| format!("fetching Chrome: {e}"))?;
    Ok(got.executable_path)
}

/// What the page reported while it ran.
#[derive(Default)]
struct Seen {
    errors: Vec<String>,
    ignored: Vec<String>,
    /// Requests by id, for a failure that names only its id.
    urls: HashMap<String, String>,
    /// The page's own responses: URL and status.
    responses: Vec<(String, i64)>,
    /// Every response by request id: URL and MIME type.
    answers: HashMap<String, (String, String)>,
    /// Every finished response: URL, MIME type and bytes on the wire.
    finished: Vec<(String, String, u64)>,
}

impl Seen {
    fn console(&mut self, what: &str, text: &str) {
        let text = redact(text);
        match ignored_console(&text) {
            Some(why) => self.ignored.push(format!("{what}: {text} ({why})")),
            None => self.errors.push(format!("{what}: {text}")),
        }
    }

    fn request(&mut self, url: &str, origin: &str, failure: &str) {
        let shown = redact(url);
        match ignored_request(url, origin) {
            Some(why) => self
                .ignored
                .push(format!("request {shown}: {failure} ({why})")),
            None => self.errors.push(format!("request {shown}: {failure}")),
        }
    }
}

/// Run the page and its checks; the summary (its checks, errors and noise) and the last screenshot of the page.
async fn smoke(a: &Args) -> Result<(Summary, Vec<u8>), String> {
    let exe = chrome(a.chrome.as_deref()).await?;
    let mut config = BrowserConfig::builder().chrome_executable(&exe);
    config = if a.headful {
        config.with_head()
    } else {
        config.new_headless_mode()
    };
    let config = config
        .window_size(WIDTH, HEIGHT)
        .viewport(Window {
            width: WIDTH,
            height: HEIGHT,
            ..Window::default()
        })
        // WebGL without a GPU (headless), and the relay's HTTP/3 (WebTransport is QUIC)
        .arg("--enable-unsafe-swiftshader")
        .arg("--use-angle=swiftshader")
        .arg("--enable-quic")
        .arg("--autoplay-policy=no-user-gesture-required")
        .user_data_dir(
            std::env::temp_dir().join(format!("arena-smoke-profile-{}", std::process::id())),
        )
        .build()?;
    let (mut browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| format!("launching {}: {e}", exe.display()))?;
    let driver = tokio::spawn(async move { while handler.next().await.is_some() {} });

    let result = drive(&browser, a).await;
    let _ = browser.close().await;
    let _ = browser.wait().await;
    driver.abort();
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("arena-smoke-profile-{}", std::process::id())),
    );
    result
}

async fn drive(browser: &Browser, a: &Args) -> Result<(Summary, Vec<u8>), String> {
    let page = browser
        .new_page("about:blank")
        .await
        .map_err(|e| e.to_string())?;
    let own = origin(&a.url);
    let seen = Arc::new(Mutex::new(Seen::default()));
    listen(&page, &seen, &own).await?;

    let mut checks = Vec::new();
    let loaded = Instant::now();
    let nav = page.goto(a.url.as_str()).await;
    if let Err(e) = nav {
        return Err(format!("loading {}: {e}", a.url));
    }

    // the game starts: its status line says the seed, live or on its own
    let deadline = Duration::from_secs(a.wait.max(5));
    let mut status = String::new();
    while loaded.elapsed() < deadline {
        status = eval_string(
            &page,
            "document.getElementById('status')?.textContent ?? ''",
        )
        .await;
        if status.starts_with("Seed") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let started = status.starts_with("Seed");
    let start_secs = loaded.elapsed().as_secs_f64();

    {
        let s = lock(&seen);
        let doc = s
            .responses
            .iter()
            .find(|(u, _)| *u == a.url)
            .map(|(_, c)| *c);
        let wasm = s
            .responses
            .iter()
            .find(|(u, _)| u.ends_with("/arena_game_bg.wasm"))
            .map(|(_, c)| *c);
        let pass = doc == Some(200) && wasm == Some(200) && started;
        checks.push(Check::new(
            "page",
            pass,
            format!(
                "page {}, game wasm {}, game {}",
                doc.map_or("not seen".into(), |c| c.to_string()),
                wasm.map_or("not seen".into(), |c| c.to_string()),
                if started {
                    format!("started in {start_secs:.1} s")
                } else {
                    format!(
                        "not started after {start_secs:.0} s (status line: {:?})",
                        redact(&status)
                    )
                }
            ),
        ));
    }

    // live.json, as the page fetches it (the URL's token is never shown)
    let lj = eval_json(
        &page,
        "fetch('live.json', {cache: 'no-store'}).then(async r => { let url = null; try { url = (await r.json()).url } catch {} \
         return {status: r.status, url: typeof url === 'string' && url.length > 0, host: typeof url === 'string' ? new URL(url).host : null} })",
    )
    .await;
    let lj_status = lj.get("status").and_then(Value::as_i64).unwrap_or(0);
    let lj_url = lj.get("url").and_then(Value::as_bool).unwrap_or(false);
    let relay = lj
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    checks.push(Check::new(
        "live.json",
        lj_status == 200 && lj_url,
        format!(
            "{lj_status}, {}",
            if lj_url {
                format!("relay {relay}")
            } else {
                "no url".into()
            }
        ),
    ));

    // let the page settle on the relay's match (it falls back to its own fight after 8 s without one)
    let settle = Duration::from_secs(10);
    let mut live = Value::Null;
    let mut first_frame = None;
    while loaded.elapsed() < deadline.max(settle) {
        live = eval_json(&page, "window.__arena?.live ?? null").await;
        if first_frame.is_none() && live.get("frames").and_then(Value::as_u64).unwrap_or(0) > 0 {
            first_frame = Some(loaded.elapsed().as_secs_f64());
        }
        status = eval_string(
            &page,
            "document.getElementById('status')?.textContent ?? ''",
        )
        .await;
        let frames = live.get("frames").and_then(Value::as_u64).unwrap_or(0);
        if frames > 0 && status.contains("· live ·") && loaded.elapsed() > Duration::from_secs(3)
        {
            break;
        }
        if loaded.elapsed() > settle && !status.contains("· live ·") && live.is_null() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let watching = status.contains("· live ·");
    let hook = !live.is_null();
    let frames0 = live.get("frames").and_then(Value::as_u64).unwrap_or(0);
    let connected = live
        .get("connected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let stale = live.get("stale").and_then(Value::as_bool).unwrap_or(false);

    // the HUD, and the fight moving: two pictures of the canvas a few seconds apart, the frames counted between
    let rect = eval_json(
        &page,
        "(() => { const r = document.getElementById('scene')?.getBoundingClientRect(); \
         return r ? {x: r.x, y: r.y, w: Math.min(r.width, innerWidth - r.x), h: Math.min(r.height, innerHeight - r.y)} : null })()",
    )
    .await;
    let clip = Viewport {
        x: rect.get("x").and_then(Value::as_f64).unwrap_or(0.0),
        y: rect.get("y").and_then(Value::as_f64).unwrap_or(0.0),
        width: rect
            .get("w")
            .and_then(Value::as_f64)
            .unwrap_or(f64::from(WIDTH))
            .max(1.0),
        height: rect
            .get("h")
            .and_then(Value::as_f64)
            .unwrap_or(f64::from(HEIGHT))
            .max(1.0),
        scale: 1.0,
    };
    let first = canvas_shot(&page, &clip).await?;
    let gap = Duration::from_secs(5);
    tokio::time::sleep(gap).await;
    let second = canvas_shot(&page, &clip).await?;
    let after = eval_json(&page, "window.__arena?.live ?? null").await;
    let frames1 = after.get("frames").and_then(Value::as_u64).unwrap_or(0);
    status = eval_string(
        &page,
        "document.getElementById('status')?.textContent ?? ''",
    )
    .await;

    let live_pass = lj_url && watching && (!hook || (connected && frames0 > 0));
    checks.push(Check::new(
        "live",
        live_pass,
        if hook {
            format!(
                "relay {}, {frames0} frames, {}",
                if connected {
                    "connected"
                } else {
                    "not connected"
                },
                if watching {
                    "watching the server's match"
                } else if stale {
                    "fighting on its own: the relay's matches are on other rules than this page (the page and arena-server must ship together)"
                } else {
                    "fighting on its own (no live match)"
                }
            )
        } else {
            format!(
                "{} (this deploy has no window.__arena: frames not counted)",
                if watching {
                    "watching the server's match"
                } else {
                    "fighting on its own (no live match)"
                }
            )
        },
    ));

    let (pa, pb) = (Picture::from_png(&first)?, Picture::from_png(&second)?);
    let painted = paint(&pb);
    checks.push(Check::new(
        "hud",
        painted.painted(),
        format!(
            "canvas {}x{}: {} colours, {:.0}% of it off the background",
            pb.width,
            pb.height,
            painted.colours,
            painted.off_background * 100.0
        ),
    ));

    let moved = changed(&pa, &pb)?;
    let pictures_moved = moved >= 0.002;
    let frames_grew = frames1 > frames0;
    let advances = if hook && watching {
        frames_grew && pictures_moved
    } else {
        pictures_moved
    };
    checks.push(Check::new(
        "advances",
        advances,
        format!(
            "{:.1}% of the fight area changed in {} s{}",
            moved * 100.0,
            gap.as_secs(),
            if hook && watching {
                format!(", frames {frames0} -> {frames1}")
            } else {
                String::new()
            }
        ),
    ));

    let full = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .build(),
        )
        .await
        .map_err(|e| format!("screenshot: {e}"))?;
    let (errors, mut ignored) = {
        let s = lock(&seen);
        (s.errors.clone(), s.ignored.clone())
    };
    ignored.dedup();
    checks.insert(
        1,
        Check::new(
            "errors",
            errors.is_empty(),
            match errors.first() {
                None => format!("none ({} ignored as noise)", ignored.len()),
                Some(first) => format!("{} (first: {first})", errors.len()),
            },
        ),
    );
    let mut weight = Weight::of(&lock(&seen).finished);
    weight.game_start_s = started.then_some(start_secs);
    weight.first_live_frame_s = first_frame;
    checks.push(Check::new("weight", true, weight.line()));
    let summary = Summary {
        checks,
        status_line: redact(&status),
        errors,
        ignored,
        weight,
        ..Summary::default()
    };
    Ok((summary, full))
}

fn lock(seen: &Arc<Mutex<Seen>>) -> std::sync::MutexGuard<'_, Seen> {
    seen.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Listen to the page's console, its uncaught exceptions and its requests.
async fn listen(page: &Page, seen: &Arc<Mutex<Seen>>, own: &str) -> Result<(), String> {
    let e = |e: chromiumoxide::error::CdpError| e.to_string();
    let mut console = page
        .event_listener::<EventConsoleApiCalled>()
        .await
        .map_err(e)?;
    let mut thrown = page
        .event_listener::<EventExceptionThrown>()
        .await
        .map_err(e)?;
    let mut sent = page
        .event_listener::<EventRequestWillBeSent>()
        .await
        .map_err(e)?;
    let mut answered = page
        .event_listener::<EventResponseReceived>()
        .await
        .map_err(e)?;
    let mut failed = page
        .event_listener::<EventLoadingFailed>()
        .await
        .map_err(e)?;
    let mut done = page
        .event_listener::<EventLoadingFinished>()
        .await
        .map_err(e)?;

    let s = Arc::clone(seen);
    tokio::spawn(async move {
        while let Some(ev) = console.next().await {
            if ev.r#type == ConsoleApiCalledType::Error || ev.r#type == ConsoleApiCalledType::Assert
            {
                let text: Vec<String> = ev
                    .args
                    .iter()
                    .map(|a| match (&a.value, &a.description) {
                        (Some(Value::String(s)), _) => s.clone(),
                        (Some(v), _) => v.to_string(),
                        (None, Some(d)) => d.clone(),
                        (None, None) => String::new(),
                    })
                    .collect();
                lock(&s).console("console error", &text.join(" "));
            }
        }
    });
    let s = Arc::clone(seen);
    tokio::spawn(async move {
        while let Some(ev) = thrown.next().await {
            let d = &ev.exception_details;
            let text = d
                .exception
                .as_ref()
                .and_then(|x| x.description.clone())
                .unwrap_or_else(|| d.text.clone());
            lock(&s).console("uncaught exception", &text);
        }
    });
    let s = Arc::clone(seen);
    tokio::spawn(async move {
        while let Some(ev) = sent.next().await {
            lock(&s)
                .urls
                .insert(ev.request_id.inner().clone(), ev.request.url.clone());
        }
    });
    let s = Arc::clone(seen);
    let o = own.to_string();
    tokio::spawn(async move {
        while let Some(ev) = answered.next().await {
            let (url, code) = (ev.response.url.clone(), ev.response.status);
            let mut s = lock(&s);
            s.answers.insert(
                ev.request_id.inner().clone(),
                (url.clone(), ev.response.mime_type.clone()),
            );
            if url.starts_with(&o) {
                s.responses.push((url.clone(), code));
            }
            if code >= 400 {
                s.request(&url, &o, &format!("answered {code}"));
            }
        }
    });
    let s = Arc::clone(seen);
    tokio::spawn(async move {
        while let Some(ev) = done.next().await {
            let mut s = lock(&s);
            if let Some((url, mime)) = s.answers.get(ev.request_id.inner()).cloned()
                && !url.starts_with("data:")
                && !url.starts_with("blob:")
            {
                // bytes on the wire: a whole number, though the protocol sends a float
                let bytes = ev.encoded_data_length.max(0.0) as u64;
                s.finished.push((url, mime, bytes));
            }
        }
    });
    let s = Arc::clone(seen);
    let o = own.to_string();
    tokio::spawn(async move {
        while let Some(ev) = failed.next().await {
            let mut s = lock(&s);
            let url = s
                .urls
                .get(ev.request_id.inner())
                .cloned()
                .unwrap_or_else(|| "(unknown)".into());
            if ev.canceled == Some(true) {
                s.ignored
                    .push(format!("request {}: canceled by the page", redact(&url)));
                continue;
            }
            s.request(&url, &o, &ev.error_text);
        }
    });
    Ok(())
}

async fn eval_json(page: &Page, js: &str) -> Value {
    match page.evaluate(js).await {
        Ok(r) => r.into_value::<Value>().unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

async fn eval_string(page: &Page, js: &str) -> String {
    eval_json(page, js).await.as_str().unwrap_or("").to_string()
}

async fn canvas_shot(page: &Page, clip: &Viewport) -> Result<Vec<u8>, String> {
    page.screenshot(
        ScreenshotParams::builder()
            .format(CaptureScreenshotFormat::Png)
            .clip(clip.clone())
            .build(),
    )
    .await
    .map_err(|e| format!("screenshot of the canvas: {e}"))
}
