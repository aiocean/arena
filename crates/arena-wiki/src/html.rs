//! The wiki's HTML: escaping, the page around a body (the site's topbar, the wiki's own menu), small builders.

use std::fmt::Write as _;

/// `s` safe in HTML text and attributes.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The wiki's sections, as its menu lists them: (file under `/wiki/`, label).
pub const MENU: &[(&str, &str)] = &[
    ("index.html", "Skills"),
    ("supports.html", "Supports"),
    ("ailments.html", "Ailments and elements"),
    ("buffs.html", "Auras, stances, domains"),
    ("curses.html", "Curses"),
    ("keystones.html", "Keystones and roots"),
    ("units.html", "Units"),
    ("looks.html", "Fighters"),
    ("rules.html", "Rules overview"),
    ("bot.html", "The bot's view"),
];

/// A whole page: `title`, the menu with `current` marked, and `body`.
pub fn page(title: &str, current: &str, body: &str) -> String {
    let mut menu = String::new();
    for (file, label) in MENU {
        let mark = if *file == current {
            " aria-current=\"page\""
        } else {
            ""
        };
        // the server answers a page without its .html (and redirects one with it)
        let href = file
            .strip_suffix(".html")
            .map_or(*file, |f| if f == "index" { "" } else { f });
        let _ = write!(menu, "<a href=\"/wiki/{href}\"{mark}>{}</a>", esc(label));
    }
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} · arena wiki</title>
  <link rel="stylesheet" href="/assets/site.css">
  <link rel="stylesheet" href="/assets/arena.css">
  <link rel="stylesheet" href="/wiki/wiki.css">
</head>
<body class="arena wiki">
  <header class="topbar">
    <a class="brand" href="/"><span class="brand-mark" aria-hidden="true"></span>arena</a>
    <nav aria-label="Site">
      <a href="/#insert">Your bot</a>
      <a href="/docs/rules">Rules</a>
      <a href="/docs/training">Train a bot</a>
      <a href="/docs/tournament">Tournament</a>
      <a href="/wiki/" aria-current="page">Wiki</a>
      <a href="https://github.com/aiocean/arena">GitHub</a>
    </nav>
  </header>
  <nav class="wiki-menu" aria-label="Wiki">{menu}</nav>
  <main class="wiki-main">
{body}
    <p class="wiki-foot">Generated from the game's code by <code>crates/arena-wiki</code> at rules version {rules}; nothing here is written by hand.</p>
  </main>
</body>
</html>
"#,
        title = esc(title),
        rules = arena_link::RULES_VERSION,
    )
}

/// A table: its header cells and its rows, every cell already HTML.
pub fn table(head: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::from("<div class=\"scroll\"><table><thead><tr>");
    for h in head {
        let _ = write!(out, "<th>{}</th>", esc(h));
    }
    out.push_str("</tr></thead><tbody>");
    for r in rows {
        out.push_str("<tr>");
        for c in r {
            let _ = write!(out, "<td>{c}</td>");
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table></div>");
    out
}

/// A definition list of (term, HTML) pairs, the empty ones left out.
pub fn facts(rows: &[(&str, String)]) -> String {
    let mut out = String::from("<dl class=\"facts\">");
    for (k, v) in rows {
        if v.is_empty() {
            continue;
        }
        let _ = write!(out, "<dt>{}</dt><dd>{v}</dd>", esc(k));
    }
    out.push_str("</dl>");
    out
}

/// A picture, lazy, at its own size in CSS pixels (`w` x `h`), scaled crisp when it is pixel art.
pub fn img(src: &str, w: u32, h: u32, alt: &str, pixel: bool) -> String {
    let class = if pixel { " class=\"pixel\"" } else { "" };
    format!(
        "<img src=\"{src}\" width=\"{w}\" height=\"{h}\" alt=\"{}\" loading=\"lazy\" decoding=\"async\"{class}>",
        esc(alt)
    )
}

/// A number as the wiki writes it: up to two decimals, no trailing zeros.
pub fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// The wiki's own styles.
pub const CSS: &str = r#"
.wiki-menu{display:flex;flex-wrap:wrap;gap:6px 14px;padding:10px 20px;border-bottom:1px solid rgba(255,255,255,.08);font-size:14px}
.wiki-menu a{color:inherit;opacity:.75;text-decoration:none}.wiki-menu a[aria-current]{opacity:1;font-weight:600;text-decoration:underline}
.wiki-main{max-width:1200px;margin:0 auto;padding:16px 20px 60px;line-height:1.55}
.wiki-main h1{font-size:26px;margin:.4em 0}.wiki-main h2{font-size:19px;margin-top:1.6em}
.wiki-main code{font-size:.9em}
.scroll{overflow-x:auto;max-width:100%}
.wiki-main table{border-collapse:collapse;font-size:14px;min-width:100%}
.wiki-main th,.wiki-main td{text-align:left;padding:5px 8px;border-bottom:1px solid rgba(255,255,255,.08);vertical-align:top}
.wiki-main th{position:sticky;top:0;background:#16131f}
.facts{display:grid;grid-template-columns:max-content 1fr;gap:4px 16px;font-size:15px}
.facts dt{opacity:.7}.facts dd{margin:0}
.strip{overflow-x:auto;padding:6px 0;background:#100e17;border-radius:6px}
.strip img{display:block;max-width:none;height:auto}
.moments{display:flex;flex-wrap:wrap;gap:10px;align-items:flex-end}
.moments figure{margin:0;max-width:100%;overflow-x:auto}
.moments img{display:block;max-width:none;height:auto;border-radius:4px}
.moments figcaption{font-size:.8em;opacity:.75}
img.pixel{image-rendering:pixelated}
.tag{display:inline-block;padding:0 7px;margin:0 4px 4px 0;border-radius:10px;background:rgba(255,255,255,.08);font-size:13px;text-decoration:none;color:inherit}
.legend span{display:inline-block;width:12px;height:12px;margin:0 4px -1px 10px;border-radius:2px}
.gen{opacity:.65;font-size:13px}
.filters{display:flex;flex-wrap:wrap;gap:8px;margin:10px 0}
.filters input,.filters select{font:inherit;padding:4px 8px;background:#16131f;color:inherit;border:1px solid rgba(255,255,255,.15);border-radius:6px}
.wiki-foot{margin-top:40px;opacity:.55;font-size:13px}
@media (max-width:640px){.facts{grid-template-columns:1fr}.facts dt{margin-top:6px}}
"#;
