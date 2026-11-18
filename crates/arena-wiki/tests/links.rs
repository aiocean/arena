//! The wiki as built: every skill of the pool has its page, and every link and picture on every page resolves (a
//! page of the wiki and its anchor, a docs page, the site's assets), and none by its .html, which the server
//! redirects.

use arena_calc::gem::GEMS;
use std::collections::BTreeSet;
use std::path::Path;

/// Every `attr="…"` value in `html`.
fn values<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
    let key = format!("{attr}=\"");
    html.match_indices(&key)
        .filter_map(|(i, _)| {
            let rest = &html[i + key.len()..];
            rest.find('"').map(|e| &rest[..e])
        })
        .collect()
}

#[test]
fn every_skill_has_a_page_and_every_link_resolves() -> Result<(), String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let wiki = arena_wiki::build(&root)?;
    for g in GEMS {
        let path = format!("skill/{}.html", g.id);
        let page = wiki
            .files
            .get(&path)
            .ok_or_else(|| format!("{} has no page", g.id))?;
        let page = std::str::from_utf8(page).map_err(|e| format!("{path}: {e}"))?;
        assert!(
            page.contains(&arena_wiki::html::esc(g.name)),
            "{path} does not name {}",
            g.name
        );
    }
    let pages: Vec<(&String, &str)> = wiki
        .files
        .iter()
        .filter(|(p, _)| p.ends_with(".html"))
        .map(|(p, b)| Ok((p, std::str::from_utf8(b).map_err(|e| format!("{p}: {e}"))?)))
        .collect::<Result<_, String>>()?;
    let ids = |html: &str| -> BTreeSet<String> {
        values(html, "id").into_iter().map(str::to_owned).collect()
    };
    let mut broken = Vec::new();
    for (path, html) in &pages {
        let links = values(html, "href").into_iter().chain(values(html, "src"));
        for link in links {
            if link.starts_with("https://") {
                continue;
            }
            let (target, frag) = link.split_once('#').unwrap_or((link, ""));
            let ok = if target.is_empty() {
                frag.is_empty() || ids(html).contains(frag)
            } else if target.ends_with(".html") {
                // the server redirects a page asked for with its .html: links go without it
                false
            } else if let Some(file) = target.strip_prefix("/wiki/") {
                let file = if file.is_empty() || file.ends_with('/') {
                    format!("{file}index.html")
                } else if file.starts_with("img/") || file.ends_with(".css") {
                    file.to_owned()
                } else {
                    format!("{file}.html")
                };
                match wiki.files.get(&file) {
                    None => false,
                    // `#tag=` is the index's filter, read by its script
                    Some(_) if frag.is_empty() || frag.starts_with("tag=") => true,
                    Some(b) => ids(std::str::from_utf8(b).unwrap_or("")).contains(frag),
                }
            } else if let Some(doc) = target.strip_prefix("/docs/") {
                root.join(format!("docs/{doc}.md")).is_file()
            } else if let Some(asset) = target.strip_prefix("/assets/") {
                root.join("site/assets").join(asset).is_file()
            } else {
                target == "/"
            };
            if !ok {
                broken.push(format!("{path}: {link}"));
            }
        }
    }
    assert!(broken.is_empty(), "broken links:\n{}", broken.join("\n"));
    Ok(())
}
