#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde_json = "1"
base64 = "0.22"
ureq = { version = "3", features = ["json"] }
---
//! jev: asks TypeSafe's Jev decision model (OpenRouter) yes/no, choice and box
//! questions about art pictures, one request a picture, so a bad image-model
//! picture (a second figure, an effect drawn into the fighter, a cut-off weapon,
//! the wrong facing) is flagged before the owner looks (art/README.md, step 5).
//!
//! Usage:
//!   cargo -Zscript art/tools/jev.rs [options] <picture.png|dir>...
//!
//! Options:
//!   --env-file <path>   read OPENROUTER_API_KEY from this file (a KEY=value line)
//!                       when it is not in the environment
//!   --checks <set>      fighter (default) | effect: which questions (QUESTIONS below)
//!   --grid <CxR>        cut each picture into C x R cells (a 4x2 sheet of key poses)
//!                       and ask about each cell
//!   --upscale <px>      enlarge a picture whose long side is under <px> by a whole
//!                       factor, nearest neighbour, before sending (default 1024:
//!                       Jev's boxes are 5x tighter on a big picture; 0 sends as is)
//!                       (a picture whose short side is under 300 px is always
//!                       enlarged by the least whole factor: Jev refuses 256 px)
//!   --bg <rrggbb>       colour a transparent picture is laid on (default ffffff)
//!   --list <file>       also read picture paths from a file, one a line
//!   --threads <n>       requests in flight (default 6)
//!   --out <file>        the JSON lines (default target/jev/answers.jsonl)
//!   --max-cost <usd>    stop sending once the spend reaches this (default 1.0)
//!   --dry-run           build the requests, send nothing
//!
//! Each line of --out: {"path","cell","sent":[w,h],"answers":{..},"latency_ms","cost"}
//! (or "error"). A noul answer is Jev's yes-probability: read >= 0.7 as yes, <= 0.3
//! as no, between as unsure. Boxes are fractions of the picture (or the cell) sent.
//! The totals (count, latency mean/p50/p90, cost) go to stdout at the end.
//!
//! The key is never printed, logged or written.

use base64::Engine;
use image::{Rgba, RgbaImage, imageops};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const URL: &str = "https://openrouter.ai/api/alpha/decisions";
const MODEL: &str = "typesafe/jev-1.14";
/// The shortest side Jev takes, with a margin (it refused 256, took 285).
const MIN_SIDE: u32 = 300;

/// What Jev is told before the picture.
const FIGHTER_STATE: &str = "The image below is one picture of a character for a 2D side-view fighting game, \
drawn on a plain background. A good picture shows exactly one character, whole, in side view facing the right \
side of the picture, with nothing drawn on or around the character but the character itself and what it holds.";
const EFFECT_STATE: &str = "The image below is one frame of a spell effect for a 2D side-view fighting game \
(a burst, a bolt, a seal, a fire), drawn on a plain background.";

/// The questions, by set. Edit the wording here.
fn questions(set: &str) -> Value {
    let facing = json!({
        "type": "choice",
        "instructions": "Which way does the character in the image face? Judge by the nose, the chest and the toes, not by where the weapon points.",
        "criteria": {
            "east": "faces the right side of the image: a side or three-quarter view looking right",
            "west": "faces the left side of the image: a side or three-quarter view looking left",
            "toward": "faces the viewer: a front view, both eyes and the whole chest seen",
            "away": "turns its back to the viewer: the back of the head and the back are seen"
        }
    });
    let one_figure = json!({
        "type": "noul",
        "instructions": "Is there exactly one character (person) in the image? Count every person or body, even a partial one at an edge; a weapon or a cloak is not a person. Answer no if there are two or more, or none."
    });
    let effect = json!({
        "type": "noul",
        "instructions": "Is a magic or visual effect drawn in the image besides the character itself: a glow, an aura, sparks, energy, fire, lightning, light rays, smoke, motion streaks or particles, on or around the character?"
    });
    let cut_off = json!({
        "type": "noul",
        "instructions": "Is any part of the character or of what it holds (the weapon, a hand, a foot, the head, hair or cloth) cut off by an edge of the image, so that it runs out of the frame?"
    });
    let feet = json!({
        "type": "noul",
        "instructions": "Is the bottom of the character inside the image: its feet or shoes (one may hide behind the other), or the hem of a long robe that covers them, not cut off by the bottom edge of the image?"
    });
    let feet_box = json!({
        "type": "box",
        "instructions": "the character's feet in the image, down to where the soles end"
    });
    let figure_box = json!({
        "type": "box",
        "instructions": "the whole character in the image, from the top of the head (or hair, hat) to the soles of the feet, with what it holds"
    });
    match set {
        "effect" => json!({
            "figure": json!({
                "type": "noul",
                "instructions": "Is there a character, a person or a body anywhere in the image?"
            }),
            "cut_off": json!({
                "type": "noul",
                "instructions": "Is the effect cut off by an edge of the image, so that part of it runs out of the frame?"
            }),
        }),
        _ => json!({
            "facing": facing,
            "one_figure": one_figure,
            "effect": effect,
            "cut_off": cut_off,
            "feet_visible": feet,
            "feet_box": feet_box,
            "figure_box": figure_box,
        }),
    }
}

struct Opts {
    env_file: Option<PathBuf>,
    set: String,
    grid: Option<(u32, u32)>,
    upscale: u32,
    bg: Rgba<u8>,
    threads: usize,
    out: PathBuf,
    max_cost: f64,
    dry_run: bool,
    pictures: Vec<PathBuf>,
}

fn usage() -> ! {
    let src = include_str!("jev.rs");
    for line in src.lines().filter(|l| l.starts_with("//!")) {
        println!(
            "{}",
            line.trim_start_matches("//!")
                .strip_prefix(' ')
                .unwrap_or("")
        );
    }
    std::process::exit(0)
}

fn parse() -> Opts {
    let mut o = Opts {
        env_file: None,
        set: "fighter".into(),
        grid: None,
        upscale: 1024,
        bg: Rgba([255, 255, 255, 255]),
        threads: 6,
        out: PathBuf::from("target/jev/answers.jsonl"),
        max_cost: 1.0,
        dry_run: false,
        pictures: vec![],
    };
    let mut args = std::env::args().skip(1);
    let need = |a: Option<String>, f: &str| a.unwrap_or_else(|| panic!("{f} needs a value"));
    while let Some(a) = args.next() {
        match a.as_str() {
            "-h" | "--help" => usage(),
            "--env-file" => o.env_file = Some(need(args.next(), &a).into()),
            "--checks" => o.set = need(args.next(), &a),
            "--grid" => {
                let v = need(args.next(), &a);
                let (c, r) = v.split_once('x').expect("--grid CxR");
                o.grid = Some((c.parse().unwrap(), r.parse().unwrap()));
            }
            "--upscale" => o.upscale = need(args.next(), &a).parse().unwrap(),
            "--bg" => {
                let v = need(args.next(), &a);
                let c = |i: usize| u8::from_str_radix(&v[i..i + 2], 16).expect("--bg rrggbb");
                o.bg = Rgba([c(0), c(2), c(4), 255]);
            }
            "--list" => {
                let f = std::fs::File::open(need(args.next(), &a)).expect("--list file");
                for l in std::io::BufReader::new(f).lines() {
                    let l = l.unwrap();
                    if !l.trim().is_empty() {
                        o.pictures.push(l.trim().into());
                    }
                }
            }
            "--threads" => o.threads = need(args.next(), &a).parse().unwrap(),
            "--out" => o.out = need(args.next(), &a).into(),
            "--max-cost" => o.max_cost = need(args.next(), &a).parse().unwrap(),
            "--dry-run" => o.dry_run = true,
            _ if a.starts_with("--") => panic!("unknown option {a} (--help)"),
            _ => o.pictures.push(a.into()),
        }
    }
    if !["fighter", "effect"].contains(&o.set.as_str()) {
        panic!("--checks fighter|effect");
    }
    o
}

fn key(env_file: &Option<PathBuf>) -> String {
    if let Ok(k) = std::env::var("OPENROUTER_API_KEY")
        && !k.trim().is_empty()
    {
        return k.trim().to_string();
    }
    if let Some(f) = env_file {
        let text = std::fs::read_to_string(f).expect("--env-file unreadable");
        for line in text.lines() {
            let line = line.trim().trim_start_matches("export ");
            if let Some(v) = line.strip_prefix("OPENROUTER_API_KEY=") {
                return v.trim().trim_matches('"').trim_matches('\'').to_string();
            }
        }
    }
    eprintln!("no OPENROUTER_API_KEY in the environment or the --env-file");
    std::process::exit(2)
}

fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = vec![];
    for p in paths {
        if p.is_dir() {
            let mut inner: Vec<PathBuf> = std::fs::read_dir(p)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            inner.sort();
            let (dirs, files): (Vec<_>, Vec<_>) = inner.into_iter().partition(|p| p.is_dir());
            out.extend(
                files
                    .into_iter()
                    .filter(|f| f.extension().is_some_and(|e| e == "png")),
            );
            out.extend(expand(&dirs));
        } else {
            out.push(p.clone());
        }
    }
    out
}

/// One question to send: a picture (or one cell of it), ready as PNG bytes.
struct Job {
    path: String,
    cell: Option<u32>,
    png: Vec<u8>,
    sent: (u32, u32),
}

fn flatten(img: &RgbaImage, bg: Rgba<u8>) -> RgbaImage {
    let mut out = RgbaImage::from_pixel(img.width(), img.height(), bg);
    for (x, y, p) in img.enumerate_pixels() {
        let a = p[3] as u32;
        let q = out.get_pixel_mut(x, y);
        for c in 0..3 {
            q[c] = ((p[c] as u32 * a + bg[c] as u32 * (255 - a)) / 255) as u8;
        }
    }
    out
}

fn prepare(img: &RgbaImage, o: &Opts) -> (Vec<u8>, (u32, u32)) {
    let mut img = flatten(img, o.bg);
    let long = img.width().max(img.height());
    let short = img.width().min(img.height()).max(1);
    // Jev refuses a small picture ("Invalid image dimensions": 256x256 refused,
    // 285x394 taken): enlarge by the least whole factor that clears MIN_SIDE
    let mut k = MIN_SIDE.div_ceil(short).max(1);
    if o.upscale > 0 && long < o.upscale {
        k = k.max(o.upscale / long);
    }
    if k > 1 {
        img = imageops::resize(
            &img,
            img.width() * k,
            img.height() * k,
            imageops::FilterType::Nearest,
        );
    }
    let mut png = vec![];
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    (png, img.dimensions())
}

fn jobs(o: &Opts) -> Vec<Job> {
    let mut out = vec![];
    for p in expand(&o.pictures) {
        let img = match image::open(&p) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                eprintln!("skip {}: {e}", p.display());
                continue;
            }
        };
        let path = p.display().to_string();
        match o.grid {
            None => {
                let (png, sent) = prepare(&img, o);
                out.push(Job {
                    path,
                    cell: None,
                    png,
                    sent,
                });
            }
            Some((c, r)) => {
                let (cw, ch) = (img.width() / c, img.height() / r);
                for i in 0..c * r {
                    let cell =
                        imageops::crop_imm(&img, (i % c) * cw, (i / c) * ch, cw, ch).to_image();
                    let (png, sent) = prepare(&cell, o);
                    out.push(Job {
                        path: path.clone(),
                        cell: Some(i),
                        png,
                        sent,
                    });
                }
            }
        }
    }
    out
}

fn ask(agent: &ureq::Agent, key: &str, body: &Value) -> Result<Value, String> {
    let mut last = String::new();
    for attempt in 0..3 {
        if attempt > 0 {
            std::thread::sleep(Duration::from_millis(1500 * attempt));
        }
        let resp = agent
            .post(URL)
            .header("Authorization", &format!("Bearer {key}"))
            .send_json(body);
        let mut resp = match resp {
            Ok(r) => r,
            Err(e) => {
                last = format!("transport: {e}");
                continue;
            }
        };
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .with_config()
            .limit(16 << 20)
            .read_to_string()
            .unwrap_or_default();
        if status == 429 || status >= 500 {
            last = format!(
                "HTTP {status}: {}",
                text.chars().take(300).collect::<String>()
            );
            continue;
        }
        if status != 200 {
            return Err(format!(
                "HTTP {status}: {}",
                text.chars().take(300).collect::<String>()
            ));
        }
        return serde_json::from_str(&text).map_err(|e| format!("bad JSON: {e}"));
    }
    Err(last)
}

fn pct(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn main() {
    let o = parse();
    if o.pictures.is_empty() {
        usage()
    }
    let all = jobs(&o);
    eprintln!("{} pictures to ask about ({} checks)", all.len(), o.set);
    let state_text = if o.set == "effect" {
        EFFECT_STATE
    } else {
        FIGHTER_STATE
    };
    let qs = questions(&o.set);
    if o.dry_run {
        for j in &all {
            println!(
                "{} {:?} {}x{} {} bytes",
                j.path,
                j.cell,
                j.sent.0,
                j.sent.1,
                j.png.len()
            );
        }
        return;
    }
    let key = key(&o.env_file);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(180)))
        .http_status_as_error(false)
        .build()
        .into();
    if let Some(d) = o.out.parent() {
        std::fs::create_dir_all(d).unwrap();
    }
    let out = Arc::new(Mutex::new(std::fs::File::create(&o.out).expect("--out")));
    let queue = Arc::new(Mutex::new(all.into_iter()));
    let stats = Arc::new(Mutex::new((Vec::<f64>::new(), 0.0f64, 0usize)));
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            let (queue, out, stats, agent, key, qs) = (&queue, &out, &stats, &agent, &key, &qs);
            let max_cost = o.max_cost;
            s.spawn(move || {
                loop {
                    if stats.lock().unwrap().1 >= max_cost {
                        return;
                    }
                    let Some(job) = queue.lock().unwrap().next() else {
                        return;
                    };
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&job.png);
                    let body = json!({
                        "model": MODEL,
                        "state": [
                            state_text,
                            {"type": "image_url", "image_url": {"url": format!("data:image/png;base64,{b64}")}}
                        ],
                        "questions": qs,
                    });
                    let t = Instant::now();
                    let r = ask(agent, key, &body);
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    let line = match r {
                        Ok(v) => {
                            let cost = v["usage"]["cost"].as_f64().unwrap_or(0.0);
                            let mut st = stats.lock().unwrap();
                            st.0.push(ms);
                            st.1 += cost;
                            json!({
                                "path": job.path, "cell": job.cell, "sent": [job.sent.0, job.sent.1],
                                "answers": v["answers"], "latency_ms": ms.round(), "cost": cost,
                                "input_tokens": v["usage"]["input_tokens"],
                            })
                        }
                        Err(e) => {
                            stats.lock().unwrap().2 += 1;
                            eprintln!("{}: {e}", job.path);
                            json!({"path": job.path, "cell": job.cell, "error": e})
                        }
                    };
                    writeln!(out.lock().unwrap(), "{line}").unwrap();
                }
            });
        }
    });
    let (mut lat, cost, errors) = stats.lock().unwrap().clone();
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = lat.iter().sum::<f64>() / lat.len().max(1) as f64;
    println!(
        "{} answered, {} errors; latency mean {:.0} ms, p50 {:.0} ms, p90 {:.0} ms; cost ${:.4}; {}",
        lat.len(),
        errors,
        mean,
        pct(&lat, 0.5),
        pct(&lat, 0.9),
        cost,
        o.out.display()
    );
}
