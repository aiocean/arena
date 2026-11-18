//! The spells' hit shapes against their drawn effects, as a review page: every skill cast once by each seat at a foe
//! who stands, then fights of house brains, and for every effect the rules play its art as the game places it beside
//! the shape the rules hit with, measured (`arena_game::audit`). Run it again after the art changes: it reads whatever
//! sheets the game's assets hold.
//!
//! ```text
//! cargo run --release -p arena-game --example spell_hitbox -- <out dir> [--seeds 12] [--ticks 3600]
//!     [--bolts crates/arena-duel/src/bolts_data.rs]
//! ```
//!
//! It writes `<out dir>/index.html` (a row an effect, `#fx-<fx with - for />` and `#spell-<skill id>` to link to), a
//! picture an effect (`<fx>.png`: a few ticks of one playing side by side, the art, its drawn box dashed grey, the
//! rules' shape red, the ground white) and `summary.json`; with `--bolts`, the bolt bodies table the rules read.

#![allow(clippy::disallowed_methods)]

use arena_game::audit::{
    ALONE, BODY, OFFSET, Report, SCALE, Sheets, audit, bolt_bodies, bolts_data, picture,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().unwrap_or_else(|| "target/spell-hitbox".into()));
    let (mut seeds, mut ticks, mut bolts) = (12u32, 3600u32, None);
    while let Some(a) = args.next() {
        let v = args.next().ok_or(format!("{a} needs a value"))?;
        match a.as_str() {
            "--seeds" => seeds = v.parse().map_err(|e| format!("--seeds: {e}"))?,
            "--ticks" => ticks = v.parse().map_err(|e| format!("--ticks: {e}"))?,
            "--bolts" => bolts = Some(PathBuf::from(v)),
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let assets = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    let mut sheets = Sheets::load(&assets)?;
    let bodies = bolt_bodies(&mut sheets);
    if let Some(path) = &bolts {
        std::fs::write(path, bolts_data(&bodies))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        println!("bolt bodies: {}", path.display());
    }
    let reports = audit(&mut sheets, 1..seeds + 1, ticks);

    let pct = |v: f64| format!("{:.0}%", v * 100.0);
    let mut anchored: BTreeSet<String> = BTreeSet::new();
    let mut row = |r: &Report, rows: &mut String| -> Result<(), String> {
        let file = format!("{}.png", r.fx.replace('/', "-"));
        let has_picture = match picture(&sheets, r, 6, 1.5) {
            Some(img) => {
                img.save(out.join(&file))
                    .map_err(|e| format!("{file}: {e}"))?;
                true
            }
            None => false,
        };
        let skills: BTreeSet<&String> = r.spells.iter().chain(&r.cast_by).collect();
        // an anchor a skill, on the first row that names it
        let anchors: String = skills
            .iter()
            .filter(|s| anchored.insert((**s).clone()))
            .map(|s| format!("<a id=\"spell-{s}\"></a>"))
            .collect();
        let scale = r.scale.map_or("–".to_string(), |(w, h)| {
            format!("{} × {}", pct(w), pct(h))
        });
        let offset = r.offset.map_or("–".to_string(), |o| format!("{o:.2}"));
        let inside = r.inside.map_or(String::new(), |i| {
            format!("<br>điểm trong hình: {}", pct(i))
        });
        let verdict = if r.ok() {
            "<b class=ok>ổn</b>".to_string()
        } else {
            r.verdict
                .iter()
                .map(|v| format!("<li>{v}</li>"))
                .collect::<Vec<_>>()
                .join("")
        };
        let _ = writeln!(
            rows,
            "<tr id=\"fx-{id}\" class={cls}><td>{anchors}<code>{fx}</code><br><small>chiêu: {skills}</small><br>\
             <small>{kind}, {n} lần chơi, hình dạng: {marks}</small></td>\
             <td>{scale}<br><small>lệch tâm {offset}{inside}</small></td>\
             <td>{art_alone} / {hit_alone}</td><td><ul>{verdict}</ul></td></tr>\
             <tr class={cls}><td colspan=4>{img}</td></tr>",
            id = r.fx.replace('/', "-"),
            cls = if r.ok() { "ok" } else { "bad" },
            fx = r.fx,
            skills = skills
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            kind = if r.drawn { "sheet vẽ" } else { "sheet bake" },
            n = r.playings,
            marks = if r.marks.is_empty() {
                "–".to_string()
            } else {
                r.marks.join(", ")
            },
            art_alone = pct(r.art_alone),
            hit_alone = pct(r.hit_alone),
            img = if has_picture {
                format!("<img src=\"{file}\" loading=lazy>")
            } else {
                String::new()
            },
        );
        Ok(())
    };
    let measured: Vec<&Report> = reports
        .iter()
        .filter(|r| !r.decoration() && !r.unplayed)
        .collect();
    let (mut off, mut fine) = (String::new(), String::new());
    for r in measured.iter().filter(|r| !r.ok()) {
        row(r, &mut off)?;
    }
    for r in measured.iter().filter(|r| r.ok()) {
        row(r, &mut fine)?;
    }
    let decorations: Vec<String> = reports
        .iter()
        .filter(|r| r.decoration())
        .map(|r| {
            let by: Vec<String> = r
                .cast_by
                .iter()
                .filter(|s| anchored.insert((*s).clone()))
                .map(|s| format!("<a id=\"spell-{s}\"></a>"))
                .collect();
            format!(
                "<span id=\"fx-{}\">{}<code>{}</code></span>",
                r.fx.replace('/', "-"),
                by.join(""),
                r.fx
            )
        })
        .collect();
    let unplayed: Vec<String> = reports
        .iter()
        .filter(|r| r.unplayed)
        .map(|r| format!("<code>{}</code>", r.fx))
        .collect();
    let bolt_rows: String = bodies
        .iter()
        .map(|(name, r)| {
            format!("<tr><td><a href=\"#spell-{name}\">{name}</a></td><td>{r:.1}</td></tr>")
        })
        .collect();

    let mut json = Vec::new();
    for r in &reports {
        json.push(serde_json::json!({
            "fx": r.fx, "spells": r.spells, "cast_by": r.cast_by, "drawn": r.drawn, "playings": r.playings,
            "marks": r.marks, "art_ticks": r.art, "hit_ticks": r.hit, "both_ticks": r.both,
            "scale": r.scale.map(|(w, h)| [w, h]), "offset": r.offset, "inside": r.inside,
            "art_alone": r.art_alone, "hit_alone": r.hit_alone, "verdict": r.verdict,
            "decoration": r.decoration(), "unplayed": r.unplayed,
        }));
    }
    let n_off = measured.iter().filter(|r| !r.ok()).count();
    let html = format!(
        "<!doctype html><html lang=vi><head><meta charset=utf-8><meta name=viewport content=\"width=device-width,initial-scale=1\">\
<title>Review: hitbox của chiêu so với hình vẽ</title><style>\
body{{font:15px/1.55 system-ui,sans-serif;background:#120e1c;color:#e8e2f0;max-width:1400px;margin:0 auto;padding:24px}}\
table{{border-collapse:collapse;width:100%}}td,th{{border:1px solid #3a3050;padding:6px 10px;vertical-align:top;text-align:left}}\
img{{max-width:100%;image-rendering:pixelated;display:block}}tr.bad td:first-child{{border-left:4px solid #ff4a4a}}\
tr.ok td:first-child{{border-left:4px solid #50ff78}}b.ok{{color:#50ff78}}ul{{margin:0;padding-left:18px}}code{{background:#241c34;padding:1px 4px}}\
.legend span{{display:inline-block;margin-right:14px}}.list code{{display:inline-block;margin:2px 6px 2px 0}}</style></head><body>\
<h1>Hitbox của chiêu so với hình vẽ của nó</h1>\
<p>Mỗi chiêu trong bộ chiêu được tung một lần bởi từng bên vào một đối thủ đứng yên (để có hình vẽ của cả hai nhân vật), \
cộng thêm {seeds} trận house brain đấu nhau (mỗi trận {secs:.0} giây). Với mỗi hiệu ứng mà luật phát, ở từng tick: hình vẽ được đặt đúng như game đặt \
(sheet vẽ theo <code>motion.json</code>, sheet bake theo ô của nó) và <b>hình dạng mà luật dùng để tính trúng</b> (<code>arena_duel::debug::shapes</code>). \
Công cụ: <code>cargo run --release -p arena-game --example spell_hitbox</code>; chạy lại sau khi vẽ lại hình.</p>\
<p class=legend><span><b style=color:#ff3232>đỏ</b>: hình dạng tính trúng của luật</span><span><b style=color:#aaaabe>xám đứt</b>: khung bao phần hình được vẽ</span><span>trắng: mặt đất</span></p>\
<p><b>Các cột:</b> <b>cỡ</b> = bề ngang × bề cao của hình vẽ so với hình dạng tính trúng (100% là khớp; ổn trong {s0:.0}–{s1:.0}%); \
<b>lệch tâm</b> = khoảng cách giữa hai tâm tính theo nửa cỡ hình dạng (ổn dưới {off}); với đạn (một điểm), <b>điểm trong hình</b> = tỉ lệ tick điểm trúng nằm trong hình vẽ. \
<b>Thời gian</b> = có vẽ mà luật không tính / luật tính mà không vẽ (ổn dưới {alone:.0}%; với đòn chỉ tính trúng một tick, như blast hay chém, hình vẽ chạy tiếp sau đó là bình thường).</p>\
<p>Theo quy định của arena-c3: hình dạng của luật là chuẩn. Chỗ lệch được sửa ở cách đặt và cỡ của hình (motion.json, px, anchor), \
trừ khi chính hình dạng của luật sai rõ ràng; khi đó arena-c3 quyết từng trường hợp. Đạn hiện được luật tính như một điểm quét theo đường bay; \
arena-27 sẽ cho đạn bán kính bằng thân hình vẽ (bảng cuối trang).</p>\
<h2>Lệch: {n_off} trên {n_measured}</h2><table><tr><th>Hiệu ứng / chiêu</th><th>Cỡ (ngang × cao)</th><th>Thời gian</th><th>Kết luận</th></tr>{off}</table>\
<h2>Khớp: {n_fine}</h2><table><tr><th>Hiệu ứng / chiêu</th><th>Cỡ (ngang × cao)</th><th>Thời gian</th><th>Kết luận</th></tr>{fine}</table>\
<h2>Chỉ trang trí: {n_dec}</h2><p>Có phát trong lúc chơi nhưng không gắn với hình dạng nào mà luật tính trúng (chớp lúc tung chiêu, tia lửa lúc trúng đòn, vòng ấn…), nên không có gì để so.</p><p class=list>{decorations}</p>\
<h2>Chưa phát: {n_unplayed}</h2><p>Có sheet vẽ nhưng không lần chạy nào phát tới (có thể là hiệu ứng của chiêu ngoài bộ chiêu, hoặc chỉ phát trong trường hợp hiếm).</p><p class=list>{unplayed}</p>\
<h2>Thân của đạn (cho arena-27)</h2><p>Nửa độ dày của thân đạn vẽ, đo vuông góc với hướng bay, tính bằng pixel của luật: lấy trung vị qua các khung của bề cao phần pixel có alpha ≥ {body}, \
chọn trường hợp dày hơn trong hai nhân vật. File <code>crates/arena-duel/src/bolts_data.rs</code> được sinh từ bảng này.</p>\
<table><tr><th>Chiêu đạn</th><th>Bán kính thân (px của luật)</th></tr>{bolt_rows}</table></body></html>",
        secs = f64::from(ticks) / 60.0,
        s0 = SCALE.0 * 100.0,
        s1 = SCALE.1 * 100.0,
        off = OFFSET,
        alone = ALONE * 100.0,
        n_measured = measured.len(),
        n_fine = measured.len() - n_off,
        n_dec = decorations.len(),
        n_unplayed = unplayed.len(),
        decorations = decorations.join(" "),
        unplayed = unplayed.join(" "),
        body = u16::from(BODY) + 1,
    );
    std::fs::write(out.join("index.html"), html).map_err(|e| e.to_string())?;
    std::fs::write(
        out.join("summary.json"),
        serde_json::to_string_pretty(&json).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    for r in &measured {
        println!(
            "{:<40} {}",
            r.fx,
            if r.ok() {
                "ok".to_string()
            } else {
                r.verdict.join("; ")
            }
        );
    }
    println!(
        "{n_off} of {} measured off, {} decorations, {} unplayed; {}",
        measured.len(),
        reports.iter().filter(|r| r.decoration()).count(),
        reports.iter().filter(|r| r.unplayed).count(),
        out.join("index.html").display()
    );
    Ok(())
}
