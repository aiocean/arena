//! `arena-smoke`: after a deploy, a headless Chrome loads the live arena site and checks that it works.
//!
//! ```text
//! cargo run --release -p arena-smoke -- [--url https://arena.aiocean.io/] [--out target/arena-smoke]
//!     [--review ~/services/arena/site/review/smoke] [--chrome <path>] [--wait 30] [--headful] [--mbps 20]
//!     [--profile phone|laptop|desktop] [--telemetry-otlp]
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
//!   files and when each finished, the fighters' sheets and the last (`arena_smoke::Weight`). The machine's own
//!   timings follow its load; `--mbps N` emulates a visitor's connection (N megabits a second down, 40 ms latency),
//!   so the seconds follow the bytes.
//! - `advances`: the match moves: the frames counter grows over a few seconds and two screenshots of the fight differ.
//! - `build`: the page's build (`window.__arena.build`, stamped by `site/build.sh`) against the live server's (the
//!   commit on its newest `match.start` in the telemetry store, read with `ARENA_OTLP_ENDPOINT` and its credentials)
//!   and their rules builds (`window.__arena.rules`, `window.__arena.live.rules`). It fails only when the rules builds
//!   differ; other commits on one rules build (a docs-only site deploy) or a build not known is a `WARN` line.
//! - `boxes`: the player's "Hitbox" toggle is there, and a click flips the hitbox overlay and another flips it back
//!   (`body[data-boxes-shown]`).
//! - `fps` (never fails the run): the page's frame rate over ten seconds of the match, from the gaps between painted
//!   frames (`requestAnimationFrame`): the rate, the median, the 95th percentile and the longest gap.
//! - `module`: the deployed house bot module (`bots/policy.wasm`, fetched beside the page) has the sha256 pinned at
//!   the deployed commit (`crates/arena-bot/tests/goldens/house_module.sha256`, read with `git show` from the clone
//!   this binary was built in, or from its working tree when its HEAD is that commit). Another hash fails; a pin not
//!   found (a commit the clone does not have, or none named) is a `WARN` line.
//!
//! `--profile` plays the page as a device (`arena_smoke::Profile`): `phone` (390x844 CSS pixels at pixel ratio 3,
//! touch, a CPU four times slower, 10 Mbit/s at 150 ms), `laptop` (1366x768, twice slower, 20 Mbit/s), `desktop`
//! (the default 1280x800, this machine). The whole page as it shows, scrolled through, is `page.png` in `--out`, the
//! canvas alone `canvas.png`. The
//! CPU throttle slows the page's own thread; WebGL is drawn by SwiftShader on this machine's CPU, unthrottled, so a
//! phone's frame rate here is a model of its script, not of its GPU.
//!
//! The wait for the game to start is a fixed 30 s (`--wait`), never stretched for a busy machine: a slow start is what
//! the run is there to see. When the game does not start within it, and only then, the whole run is tried once more
//! in a fresh browser; the result fails only when both attempts fail, and the report says that it was retried and why.
//! Other failed checks are the result at once.
//!
//! Every run measures the time from the navigation to the game starting (its status line set) and to the first live
//! match frame (`window.__arena.live.frames` > 0), and the machine's 1/5/15-minute load averages when it began. With
//! `--telemetry-otlp` (or `ARENA_OTLP_ENDPOINT`, `ops/observe/README.md`) it sends them as one `smoke.run` event
//! (service `arena-smoke`, stream `arena_smoke`) and the metrics `arena_smoke_game_start_seconds`,
//! `arena_smoke_first_frame_seconds` and `arena_smoke_runs_total` (`result`).
//!
//! It writes `screenshot.png` and `summary.json` to `--out`; with `--review <dir>` also a run page into
//! `<dir>/<UTC time>/` and the index of the last 30 runs at `<dir>/index.html`. Exit code 0 when every check passes, 1
//! when one fails, 2 when the browser could not run at all.
//!
//! Chrome: `--chrome`, else `$CHROME`, else a system Chrome or Chromium, else Chrome for Testing downloaded once into
//! `~/.cache/arena-smoke`.

use arena_smoke::{
    Check, Fps, Load, Picture, Profile, Summary, Weight, changed, ignored_console, ignored_request,
    paint, redact, stamp,
};
use arena_telemetry::target::METRICS;
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
use tracing::Level;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 800;

struct Args {
    url: String,
    out: PathBuf,
    review: Option<PathBuf>,
    chrome: Option<PathBuf>,
    wait: u64,
    headful: bool,
    /// A visitor's connection to emulate: megabits a second down, 40 ms latency (none: the machine's own).
    mbps: Option<f64>,
    /// The device to play the page as (`--profile phone|laptop|desktop`).
    profile: Profile,
}

/// The fixed wait for the game to start, seconds.
const WAIT: u64 = 30;

fn args() -> Result<Args, String> {
    let mut a = Args {
        url: "https://arena.aiocean.io/".into(),
        out: PathBuf::from("target/arena-smoke"),
        review: None,
        chrome: std::env::var_os("CHROME").map(PathBuf::from),
        wait: WAIT,
        headful: false,
        mbps: None,
        profile: Profile::DESKTOP,
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
            "--mbps" => a.mbps = Some(value()?.parse().map_err(|e| format!("--mbps: {e}"))?),
            "--profile" => {
                let name = value()?;
                a.profile = Profile::named(&name)
                    .ok_or(format!("--profile {name}: phone, laptop or desktop"))?;
            }
            // read by arena_telemetry::requested
            arena_telemetry::FLAG => {}
            "-h" | "--help" => {
                return Err("usage: arena-smoke [--url URL] [--out DIR] [--review DIR] [--chrome PATH] [--wait SECONDS] [--headful] [--mbps N] [--profile phone|laptop|desktop] [--telemetry-otlp]".into());
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

fn main() -> ExitCode {
    let a = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let raw: Vec<String> = std::env::args().collect();
    let export = match arena_telemetry::requested(&raw) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    // the browser driver's own chatter stays out of stderr and the stream
    let telemetry = export.then(|| {
        arena_telemetry::init(
            "arena-smoke",
            arena_telemetry::Options {
                filter: "warn,chromiumoxide=off".into(),
                export_filter: "warn,arena_smoke=info,chromiumoxide=off".into(),
                ..arena_telemetry::Options::tool(env!("CARGO_PKG_VERSION"))
            },
        )
    });
    let code = run(&a);
    if let Some(t) = &telemetry {
        t.flush();
        if t.exporting() {
            println!(
                "telemetry: smoke.run sent to {} (stream arena_smoke)",
                t.endpoint().unwrap_or("")
            );
        }
    }
    code
}

fn run(a: &Args) -> ExitCode {
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("arena-smoke could not start its runtime: {e}");
            return ExitCode::from(2);
        }
    };
    let at = stamp(SystemTime::now());
    let begun = Instant::now();
    let stamped = |mut s: Summary| {
        s.at = at.clone();
        s.url = a.url.clone();
        s
    };
    let (first, first_shot) = match rt.block_on(attempt(a, 1)) {
        Ok((s, shot)) => (stamped(s), shot),
        Err(e) => {
            eprintln!("arena-smoke could not run: {}", redact(&e));
            let mut s = stamped(Summary {
                attempt: 1,
                load: Load::now(),
                ..Summary::default()
            });
            s.seconds = begun.elapsed().as_secs_f64();
            record(&s, "error", a.mbps);
            return ExitCode::from(2);
        }
    };
    let (mut summary, shot, retried_shot) = match first.retry() {
        None => (first, first_shot, None),
        Some(why) => {
            println!("RETRY {why}");
            for c in &first.checks {
                println!(
                    "  attempt 1: {} {:<10} {}",
                    if c.pass { "PASS" } else { "FAIL" },
                    c.name,
                    c.detail
                );
            }
            println!("  attempt 1: {}", first.timings());
            match rt.block_on(attempt(a, 2)) {
                Ok((second, shot)) => (
                    Summary::retried(first, stamped(second)),
                    shot,
                    Some(first_shot),
                ),
                Err(e) => {
                    // the second could not run at all: the first's failure stands
                    let mut s = first;
                    s.attempt = 2;
                    s.retry_reason = Some(format!(
                        "{why}; the second attempt could not run: {}",
                        redact(&e)
                    ));
                    (s, first_shot, None)
                }
            }
        }
    };
    summary.seconds = begun.elapsed().as_secs_f64();

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
    println!("timings: {}", summary.timings());
    if let Some(why) = &summary.retry_reason {
        println!("retried: {why}");
    }
    println!(
        "deployed commit: {}{} · server build: {}",
        summary.deployed_commit.as_deref().unwrap_or("none named"),
        if summary.deployed_dirty { "+dirty" } else { "" },
        summary.server_build.as_deref().unwrap_or("unknown")
    );
    if let Some(w) = &summary.build_warning {
        println!("WARN {w}");
    }
    if let Some(w) = &summary.module_warning {
        println!("WARN {w}");
    }
    let json = serde_json::to_string_pretty(&summary).unwrap_or_default();
    let write = || -> Result<(), String> {
        std::fs::create_dir_all(&a.out).map_err(|e| format!("{}: {e}", a.out.display()))?;
        std::fs::write(a.out.join("screenshot.png"), &shot).map_err(|e| e.to_string())?;
        std::fs::write(a.out.join("summary.json"), &json).map_err(|e| e.to_string())?;
        if let Some(first) = &retried_shot {
            std::fs::write(a.out.join("attempt-1.png"), first).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    if let Err(e) = write() {
        eprintln!("writing the results: {e}");
    } else {
        println!("screenshot and summary: {}", a.out.display());
    }
    if let Some(dir) = &a.review {
        match arena_smoke::write_review(dir, &summary, &shot, retried_shot.as_deref()) {
            Ok(run) => println!("review: {}", run.display()),
            Err(e) => eprintln!("writing the review: {e}"),
        }
    }
    record(&summary, if summary.pass { "pass" } else { "fail" }, a.mbps);
    println!("{}", if summary.pass { "PASS" } else { "FAIL" });
    if summary.pass {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

/// One attempt: the machine's load as it begins, then the page and its checks in a fresh browser.
async fn attempt(a: &Args, n: u32) -> Result<(Summary, Vec<u8>), String> {
    let load = Load::now();
    let (mut s, shot) = smoke(a).await?;
    s.attempt = n;
    s.load = load;
    s.finish();
    Ok((s, shot))
}

/// The run as telemetry (exported only when `init` set an endpoint): one `smoke.run` event on the service's stream,
/// the two timings as histograms and the run counted by result.
fn record(s: &Summary, result: &str, mbps: Option<f64>) {
    let failed: Vec<&str> = s
        .checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| c.name.as_str())
        .collect();
    tracing::info!(
        run_id = arena_telemetry::run_id("smoke"),
        at = %s.at,
        url = %s.url,
        result = result,
        check_page = s.check("page"),
        check_errors = s.check("errors"),
        check_live_json = s.check("live.json"),
        check_live = s.check("live"),
        check_hud = s.check("hud"),
        check_boxes = s.check("boxes"),
        check_advances = s.check("advances"),
        check_build = s.check("build"),
        check_module = s.check("module"),
        failed = %failed.join(","),
        game_start_seconds = s.game_start_seconds,
        first_frame_seconds = s.first_frame_seconds,
        load_1 = s.load.map(|l| l.one),
        load_5 = s.load.map(|l| l.five),
        load_15 = s.load.map(|l| l.fifteen),
        attempt = s.attempt,
        retried = s.retry_reason.is_some(),
        // always set, so the stream always has the fields a dashboard names
        retry_reason = s.retry_reason.as_deref().unwrap_or(""),
        deployed_commit = s.deployed_commit.as_deref().unwrap_or("none"),
        deployed_dirty = s.deployed_dirty,
        server_build = s.server_build.as_deref().unwrap_or("unknown"),
        page_rules = s.page_rules,
        server_rules = s.server_rules,
        build_warning = s.build_warning.as_deref().unwrap_or(""),
        module_hash = s.module_hash.as_deref().unwrap_or("unknown"),
        module_warning = s.module_warning.as_deref().unwrap_or(""),
        errors = s.errors.len(),
        seconds = s.seconds,
        // the connection emulated (`--mbps`), 0 for the machine's own: the timings follow it
        mbps = mbps.unwrap_or(0.0),
        status_line = %s.status_line,
        "smoke.run"
    );
    if let Some(v) = s.game_start_seconds {
        tracing::event!(target: METRICS, Level::INFO, { histogram.arena_smoke_game_start_seconds = v });
    }
    if let Some(v) = s.first_frame_seconds {
        tracing::event!(target: METRICS, Level::INFO, { histogram.arena_smoke_first_frame_seconds = v });
    }
    tracing::event!(target: METRICS, Level::INFO, { monotonic_counter.arena_smoke_runs_total = 1_u64, result = result });
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
    /// Every finished response: URL, MIME type, bytes on the wire and the browser's clock at its last byte.
    finished: Vec<(String, String, u64, f64)>,
    /// The browser's clock (seconds) at the first request, and at the last response finished.
    first_sent: Option<f64>,
    last_done: f64,
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
    let p = &a.profile;
    let config = config
        .window_size(p.width.max(WIDTH), p.height.max(HEIGHT))
        .viewport(Window {
            width: p.width,
            height: p.height,
            device_scale_factor: Some(p.dpr),
            emulating_mobile: p.mobile,
            is_landscape: p.width > p.height,
            has_touch: p.mobile,
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
    // the connection: --mbps, else the profile's
    if let Some(mbps) = a.mbps.or(a.profile.mbps) {
        // deprecated for the by-rule commands, which chromiumoxide 0.9 does not have yet
        #[allow(deprecated)]
        let throttle =
            chromiumoxide::cdp::browser_protocol::network::EmulateNetworkConditionsParams::new(
                false,
                a.profile.rtt_ms,
                mbps * 125_000.0,
                mbps * 125_000.0,
            );
        page.execute(throttle)
            .await
            .map_err(|e| format!("emulating {mbps} Mbit/s: {e}"))?;
    }
    if a.profile.cpu > 1.0 {
        page.execute(
            chromiumoxide::cdp::browser_protocol::emulation::SetCpuThrottlingRateParams::new(
                a.profile.cpu,
            ),
        )
        .await
        .map_err(|e| format!("a CPU {}x slower: {e}", a.profile.cpu))?;
    }

    let mut checks = Vec::new();
    let loaded = Instant::now();
    let nav = page.goto(a.url.as_str()).await;
    if let Err(e) = nav {
        return Err(format!("loading {}: {e}", a.url));
    }

    // the game starts: its status line says the seed, live or on its own
    let deadline = Duration::from_secs(a.wait.max(5));
    let mut status = String::new();
    // when the first live frame was seen, from the navigation
    let mut first_frame: Option<f64> = None;
    while loaded.elapsed() < deadline {
        status = eval_string(
            &page,
            "document.getElementById('status')?.textContent ?? ''",
        )
        .await;
        if first_frame.is_none() && live_frames(&page).await > 0 {
            first_frame = Some(loaded.elapsed().as_secs_f64());
        }
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
        "fetch('live.json', {cache: 'no-store'}).then(async r => { let url = null, commit = null; \
         try { const j = await r.json(); url = j.url; commit = j.build?.commit ?? j.commit ?? j.version ?? null } catch {} \
         return {status: r.status, url: typeof url === 'string' && url.length > 0, host: typeof url === 'string' ? new URL(url).host : null, \
         commit: commit == null ? null : String(commit)} })",
    )
    .await;
    // the deployed commit: the page's build stamp (site/build.sh), else what live.json names
    let named = eval_json(
        &page,
        "(() => { const v = window.__arena?.build?.commit ?? window.__arena?.commit; return v == null ? null : String(v) })()",
    )
    .await;
    let deployed_dirty = eval_json(&page, "window.__arena?.build?.dirty === true")
        .await
        .as_bool()
        .unwrap_or(false);
    let deployed_commit = named
        .as_str()
        .or_else(|| lj.get("commit").and_then(Value::as_str))
        .map(|c| redact(c.trim()))
        .filter(|c| !c.is_empty());
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
    while loaded.elapsed() < deadline.max(settle) {
        live = eval_json(&page, "window.__arena?.live ?? null").await;
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
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let watching = status.contains("· live ·");
    let hook = !live.is_null();
    let frames0 = live.get("frames").and_then(Value::as_u64).unwrap_or(0);
    let connected = live
        .get("connected")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let stale = live.get("stale").and_then(Value::as_bool).unwrap_or(false);
    // the rules builds: the page's own, and the one the server's keyframe was fought on (0 before one came)
    let page_rules = eval_json(&page, "window.__arena?.rules ?? null")
        .await
        .as_u64()
        .and_then(|r| u32::try_from(r).ok());
    let server_rules = live
        .get("rules")
        .and_then(Value::as_u64)
        .and_then(|r| u32::try_from(r).ok())
        .filter(|&r| r > 0);
    let server_build = tokio::task::spawn_blocking(server_build)
        .await
        .unwrap_or(None);

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
    // the fight as the canvas showed it, beside the window's screenshot
    let _ = std::fs::create_dir_all(&a.out);
    let _ = std::fs::write(a.out.join("canvas.png"), &second);
    let painted = paint(&pb);
    // the canvas as the page lays it out (CSS pixels) and as the game draws it (its backing pixels)
    let canvas = eval_string(
        &page,
        "(() => { const c = document.getElementById('scene'); return c ? \
         `${c.clientWidth}x${c.clientHeight} CSS px, drawn at ${c.width}x${c.height}, pixel ratio ${devicePixelRatio}` : '' })()",
    )
    .await;
    checks.push(Check::new(
        "hud",
        painted.painted(),
        format!(
            "canvas {canvas}: {} colours, {:.0}% of it off the background",
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

    // the hitbox overlay's toggle: there, and it turns the overlay on and off again
    let toggled = eval_string(
        &page,
        "(() => { const b = document.querySelector('button[data-boxes]'); if (!b) return 'no toggle'; \
         const was = document.body.dataset.boxesShown; b.click(); const flipped = document.body.dataset.boxesShown; b.click(); b.blur(); window.scrollTo(0, 0); \
         return was !== flipped && document.body.dataset.boxesShown === was ? 'ok' : `${was} -> ${flipped} -> ${document.body.dataset.boxesShown}`; })()",
    )
    .await;
    checks.push(Check::new(
        "boxes",
        toggled == "ok",
        if toggled == "ok" {
            "the Hitbox toggle turns the overlay on and off".to_string()
        } else {
            format!("the Hitbox toggle did not flip and flip back: {toggled}")
        },
    ));
    // the frame rate over ten seconds of the match, from the gaps between painted frames
    let gaps = eval_json(
        &page,
        "new Promise(done => { const gaps = []; let last = null; const t0 = performance.now(); \
         const step = t => { if (last !== null) gaps.push(t - last); last = t; \
         if (t - t0 < 10000) requestAnimationFrame(step); else done(gaps); }; requestAnimationFrame(step); })",
    )
    .await;
    let gaps: Vec<f64> = gaps
        .as_array()
        .map(|g| g.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let fps = Fps::of(&gaps);
    checks.push(Check::new(
        "fps",
        true,
        fps.as_ref()
            .map_or("no frames painted".to_string(), Fps::line),
    ));
    // the window's picture from the top of the page (a phone's page may have scrolled under the run)
    let _ = eval_json(&page, "(() => { window.scrollTo(0, 0); return true })()").await;
    let full = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .build(),
        )
        .await
        .map_err(|e| format!("screenshot: {e}"))?;
    // the whole page as the device shows it, below the window too, without resizing the window (a full-page
    // capture that resizes it stretches the game, which fills the window's height)
    let size = eval_json(
        &page,
        "({w: document.documentElement.scrollWidth, h: document.documentElement.scrollHeight})",
    )
    .await;
    let whole = page
        .screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .clip(Viewport {
                    x: 0.0,
                    y: 0.0,
                    width: size
                        .get("w")
                        .and_then(Value::as_f64)
                        .unwrap_or(f64::from(WIDTH)),
                    height: size
                        .get("h")
                        .and_then(Value::as_f64)
                        .unwrap_or(f64::from(HEIGHT)),
                    scale: 1.0,
                })
                .capture_beyond_viewport(true)
                .build(),
        )
        .await;
    if let Ok(whole) = whole {
        let _ = std::fs::create_dir_all(&a.out);
        let _ = std::fs::write(a.out.join("page.png"), whole);
    }

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
    let weight = {
        let s = lock(&seen);
        let t0 = s.first_sent.unwrap_or(0.0);
        let since: Vec<_> = s
            .finished
            .iter()
            .map(|(u, m, b, at)| (u.clone(), m.clone(), *b, at - t0))
            .collect();
        let mut w = Weight::of(&since);
        w.all_loaded_s = s.first_sent.map(|t| s.last_done - t);
        w
    };
    checks.push(Check::new("weight", true, weight.line()));
    let (build, build_warning) = arena_smoke::compare_builds(
        deployed_commit.as_deref(),
        server_build.as_deref(),
        page_rules,
        server_rules,
    );
    checks.push(build);
    let module_url = arena_smoke::module_url(&a.url);
    let commit = deployed_commit.clone();
    let (module_hash, pinned) = tokio::task::spawn_blocking(move || {
        (
            deployed_module(&module_url),
            commit.as_deref().and_then(pinned_module),
        )
    })
    .await
    .map_err(|e| e.to_string())?;
    let (module, module_warning) = arena_smoke::compare_module(
        module_hash.as_deref().map_err(String::as_str),
        pinned.as_ref().map(|(h, from)| (h.as_str(), from.as_str())),
        deployed_dirty,
    );
    checks.push(module);
    let summary = Summary {
        checks,
        status_line: redact(&status),
        errors,
        ignored,
        weight,
        start_timed_out: !started,
        game_start_seconds: started.then_some(start_secs),
        first_frame_seconds: first_frame,
        deployed_commit,
        deployed_dirty,
        server_build,
        page_rules,
        server_rules,
        build_warning,
        profile: a.profile.name.to_string(),
        fps,
        module_hash: module_hash.ok(),
        module_warning,
        ..Summary::default()
    };
    Ok((summary, full))
}

/// The live server's build: the `build` on its newest `match.start` of the last half hour in the telemetry store
/// (OpenObserve's `_search`, with `ARENA_OTLP_ENDPOINT` and its credentials), or None when there is no store, no such
/// row, or it could not be read.
fn server_build() -> Option<String> {
    let endpoint = arena_telemetry::Endpoint::from_env().ok()??;
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_micros() as i64;
    let sql = "SELECT build FROM arena_match WHERE body = 'match.start' AND service_name = 'arena-server' \
               AND build IS NOT NULL ORDER BY _timestamp DESC LIMIT 1";
    let body = serde_json::json!({"query": {"sql": sql, "start_time": now - 1_800_000_000, "end_time": now, "from": 0, "size": 1}});
    let url = format!("{}/_search?type=logs", endpoint.url);
    let mut req = ureq::post(&url)
        .config()
        .timeout_global(Some(Duration::from_secs(10)))
        .build();
    if let Some(auth) = &endpoint.auth {
        req = req.header("Authorization", auth);
    }
    let v: Value = req.send_json(&body).ok()?.body_mut().read_json().ok()?;
    v["hits"][0]["build"]
        .as_str()
        .map(str::to_string)
        .filter(|b| !b.is_empty())
}

/// The deployed house bot module's sha256: fetched as a visitor's browser would, uncompressed.
fn deployed_module(url: &str) -> Result<String, String> {
    let mut res = ureq::get(url)
        .config()
        .timeout_global(Some(Duration::from_secs(60)))
        .build()
        .call()
        .map_err(|e| format!("{url}: {e}"))?;
    let bytes = res
        .body_mut()
        .with_config()
        .limit(256 << 20)
        .read_to_vec()
        .map_err(|e| format!("{url}: {e}"))?;
    Ok(arena_smoke::sha256_hex(&bytes))
}

/// The house module's hash pinned at `commit`, and where it was read: `git show` in the clone this binary was built
/// in, else its working tree's golden when the clone's HEAD is that commit; `None` when neither has it.
fn pinned_module(commit: &str) -> Option<(String, String)> {
    const GOLDEN: &str = "crates/arena-bot/tests/goldens/house_module.sha256";
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&repo)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    if !commit.is_empty()
        && commit.chars().all(|c| c.is_ascii_hexdigit())
        && let Some(h) = git(&["show", &format!("{commit}:{GOLDEN}")]).filter(|h| !h.is_empty())
    {
        return Some((h, format!("{GOLDEN} at {commit}")));
    }
    let head = git(&["rev-parse", "HEAD"])?;
    if !arena_smoke::same_commit(&head, commit) {
        return None;
    }
    let h = std::fs::read_to_string(repo.join(GOLDEN)).ok()?;
    let h = h.trim().to_string();
    (!h.is_empty()).then(|| (h, format!("{GOLDEN} in the clone at {commit}")))
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
            let mut s = lock(&s);
            s.first_sent.get_or_insert(*ev.timestamp.inner());
            s.urls
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
                let at = *ev.timestamp.inner();
                s.finished.push((url, mime, bytes, at));
                s.last_done = s.last_done.max(at);
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

/// The live match frames the page has taken (`window.__arena.live.frames`), 0 before any or without the hook.
async fn live_frames(page: &Page) -> u64 {
    eval_json(page, "window.__arena?.live?.frames ?? 0")
        .await
        .as_u64()
        .unwrap_or(0)
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
