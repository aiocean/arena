//! The words the code already has for its skills and supports: a skill's comment over its row in the pool
//! (`arena-calc/src/gem.rs`) and the section it sits in, and a support's "what it does" from the design's table
//! (`docs/specs/2026-11-17-damage-system-design.md`, §8). Read from the files as they are; nothing is made up here.

use std::collections::BTreeMap;

/// A skill's words: its section of the pool and the comment over its row, if it has one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkillWords {
    pub section: String,
    pub comment: Option<String>,
}

/// Each skill's words, by id, from the pool's source.
pub fn skills(gem_rs: &str) -> BTreeMap<String, SkillWords> {
    let mut out = BTreeMap::new();
    let mut inside = false;
    let mut section = String::new();
    let mut comment: Vec<String> = Vec::new();
    for line in gem_rs.lines() {
        let t = line.trim();
        if !inside {
            inside = t.starts_with("pub const ALL: &[Gem]");
            continue;
        }
        if t == "];" {
            break;
        }
        if let Some(c) = t.strip_prefix("//") {
            let c = c.trim();
            if let Some(s) = c.strip_prefix("----") {
                section = s.trim_end_matches('-').trim().to_owned();
                comment.clear();
            } else if !c.is_empty() {
                comment.push(c.to_owned());
            }
            continue;
        }
        if let Some(id) = first_literal(t) {
            out.insert(
                id.to_owned(),
                SkillWords {
                    section: section.clone(),
                    comment: (!comment.is_empty()).then(|| comment.join(" ")),
                },
            );
        }
        comment.clear();
    }
    out
}

/// The first string literal on a row: the gem's id (`attack("slash", "Chém", ...)`).
fn first_literal(line: &str) -> Option<&str> {
    let start = line.find("(\"")? + 2;
    let len = line[start..].find('"')?;
    Some(&line[start..start + len])
}

/// A support's row in the design's table: what it asks of a skill and what it does, in the design's words.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SupportWords {
    pub asks: String,
    pub does: String,
}

/// Each support's words, by id, from the design's table (`| \`id\` | **Name** | asks | does | ...`).
pub fn supports(design_md: &str) -> BTreeMap<String, SupportWords> {
    let mut out = BTreeMap::new();
    for line in design_md.lines() {
        let Some(rest) = line.strip_prefix("| `") else {
            continue;
        };
        let cells: Vec<&str> = rest.split('|').map(str::trim).collect();
        if cells.len() < 4 || !cells[1].starts_with("**") {
            continue;
        }
        let id = cells[0].trim_end_matches('`');
        out.insert(
            id.to_owned(),
            SupportWords {
                asks: cells[2].to_owned(),
                does: cells[3].to_owned(),
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rows_comment_is_its_skills_words() {
        let src = "x\n    pub const ALL: &[Gem] = &[\n        // ---- 11.1 basics ----\n        attack(\"slash\", \"Chém\"),\n        // Pays life.\n        // Twice.\n        rank(with(attack(\"fist\", \"Quyền\"))),\n    ];\n";
        let w = skills(src);
        assert_eq!(
            w.get("slash").map(|s| s.section.as_str()),
            Some("11.1 basics")
        );
        assert_eq!(w.get("slash").and_then(|s| s.comment.clone()), None);
        assert_eq!(
            w.get("fist").and_then(|s| s.comment.clone()).as_deref(),
            Some("Pays life. Twice.")
        );
    }

    #[test]
    fn a_supports_row_is_read_by_its_id() {
        let md = "| id | Bí quyết | Đòi tag | Làm gì |\n|---|---|---|---|\n| `focus` | **Thanh Trần** | any | nhân +25% | 1,2 | 2 |\n";
        let w = supports(md);
        assert_eq!(w.get("focus").map(|s| s.does.as_str()), Some("nhân +25%"));
    }
}
