//! A fighter's sprite sheet: one picture or more (pages, each under the 4096 pixels a side a browser takes) holding
//! every frame, and its layout (`<name>.json`, written by `art/cultivator/tools/sheet.rs`): where each animation's
//! frames sit, on which page, and where the feet are in each.

use serde::Deserialize;
use std::collections::BTreeMap;

/// One frame on the sheet: the page it is on (the first when left out), its box there (`x`, `y`, `w`, `h`) and where
/// its top-left sits from the feet (`dx`, `dy`), facing right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    #[serde(default)]
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub dx: i32,
    pub dy: i32,
}

/// Where everything is on a sheet: the size of each page, each animation's frames in order, the face for the HUD's
/// medallion (a box on the sheet) and the frames a second each animation plays at. A sheet of one page may give its
/// `size` instead of `pages`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(from = "RawLayout")]
pub struct Layout {
    pub pages: Vec<[u32; 2]>,
    pub fps: f64,
    pub clips: BTreeMap<String, Vec<Cell>>,
    /// A fighter's face; an effect's sheet has none.
    pub portrait: Option<Cell>,
    /// Whether each animation plays round and round (an effect held until it is stopped) or once.
    pub looping: bool,
}

/// The layout as written: `pages`, or the `size` of its one page.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayout {
    #[serde(default)]
    size: Option<[u32; 2]>,
    #[serde(default)]
    pages: Vec<[u32; 2]>,
    fps: f64,
    clips: BTreeMap<String, Vec<Cell>>,
    #[serde(default)]
    portrait: Option<Cell>,
    #[serde(default)]
    looping: bool,
}

impl From<RawLayout> for Layout {
    fn from(r: RawLayout) -> Layout {
        let pages = if r.pages.is_empty() {
            r.size.into_iter().collect()
        } else {
            r.pages
        };
        Layout {
            pages,
            fps: r.fps,
            clips: r.clips,
            portrait: r.portrait,
            looping: r.looping,
        }
    }
}

impl Layout {
    /// How many RGBA bytes its pages hold together.
    pub fn bytes(&self) -> usize {
        self.pages
            .iter()
            .map(|[w, h]| *w as usize * *h as usize * 4)
            .sum()
    }
}

#[derive(Debug, PartialEq)]
pub enum LayoutError {
    Json(String),
    /// The pictures are not the bytes the layout's pages hold (`want`), or a one-page sheet's picture is not the
    /// size it says.
    Size {
        want: [u32; 2],
        got: [u32; 2],
        bytes: usize,
    },
    /// A frame's box reaches past the sheet.
    Outside {
        clip: String,
        frame: usize,
    },
    /// An animation the fighter needs is missing or has no frames.
    Missing(String),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::Json(e) => write!(f, "sheet layout: {e}"),
            LayoutError::Size { want, got, bytes } => write!(
                f,
                "sheet picture is {}x{} ({bytes} bytes), the layout says {}x{}",
                got[0], got[1], want[0], want[1]
            ),
            LayoutError::Outside { clip, frame } => {
                write!(f, "sheet layout: {clip}[{frame}] reaches past the sheet")
            }
            LayoutError::Missing(clip) => write!(f, "sheet layout: no frames for \"{clip}\""),
        }
    }
}

impl std::error::Error for LayoutError {}

/// A sheet ready to draw from: each page's straight-alpha RGBA pixels and their layout, checked once when it is
/// made.
#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub layout: Layout,
    pages: Vec<Vec<[u8; 4]>>,
}

impl Sheet {
    /// A one-page sheet from its layout's JSON and its picture's RGBA bytes (`w` x `h`).
    pub fn new(layout_json: &str, rgba: &[u8], w: u32, h: u32) -> Result<Sheet, LayoutError> {
        let sheet = Sheet::paged(layout_json, rgba)?;
        if sheet.layout.pages != [[w, h]] {
            let want = sheet.layout.pages.first().copied().unwrap_or([0, 0]);
            return Err(LayoutError::Size {
                want,
                got: [w, h],
                bytes: rgba.len(),
            });
        }
        Ok(sheet)
    }

    /// A fighter's sheet from its layout's JSON and every page's RGBA bytes one after another, in the layout's order.
    /// Every animation [`crate::pick`] can ask for must be there, and the face.
    pub fn paged(layout_json: &str, rgba: &[u8]) -> Result<Sheet, LayoutError> {
        let sheet = Sheet::any(layout_json, rgba)?;
        if sheet.layout.portrait.is_none() {
            return Err(LayoutError::Missing("portrait".into()));
        }
        for clip in crate::pick::CLIPS {
            if sheet.layout.clips.get(*clip).is_none_or(Vec::is_empty) {
                return Err(LayoutError::Missing((*clip).into()));
            }
        }
        Ok(sheet)
    }

    /// Any sheet (an effect's: its animations are what it has) from its layout's JSON and every page's RGBA bytes one
    /// after another, in the layout's order.
    pub fn any(layout_json: &str, rgba: &[u8]) -> Result<Sheet, LayoutError> {
        let layout: Layout =
            serde_json::from_str(layout_json).map_err(|e| LayoutError::Json(e.to_string()))?;
        if layout.pages.is_empty() || rgba.len() != layout.bytes() {
            let px = (layout.bytes() / 4) as u32;
            return Err(LayoutError::Size {
                want: [px, 1],
                got: [(rgba.len() / 4) as u32, 1],
                bytes: rgba.len(),
            });
        }
        let pages_ = &layout.pages;
        let inside = |c: &Cell| {
            pages_
                .get(c.page as usize)
                .is_some_and(|[w, h]| c.x + c.w <= *w && c.y + c.h <= *h)
        };
        for (name, cells) in &layout.clips {
            if let Some(frame) = cells.iter().position(|c| !inside(c)) {
                return Err(LayoutError::Outside {
                    clip: name.clone(),
                    frame,
                });
            }
        }
        if layout.portrait.is_some_and(|p| !inside(&p)) {
            return Err(LayoutError::Outside {
                clip: "portrait".into(),
                frame: 0,
            });
        }
        let (chunks, _) = rgba.as_chunks::<4>();
        let mut pages = Vec::with_capacity(layout.pages.len());
        let mut at = 0;
        for [w, h] in &layout.pages {
            let n = *w as usize * *h as usize;
            pages.push(chunks[at..at + n].to_vec());
            at += n;
        }
        Ok(Sheet { layout, pages })
    }

    /// Pixel (`x`, `y`) of page `page`, straight alpha; transparent off it.
    pub fn at(&self, page: u32, x: u32, y: u32) -> [u8; 4] {
        let (Some([w, h]), Some(px)) = (
            self.layout.pages.get(page as usize),
            self.pages.get(page as usize),
        ) else {
            return [0; 4];
        };
        if x >= *w || y >= *h {
            return [0; 4];
        }
        px.get((y * w + x) as usize).copied().unwrap_or([0; 4])
    }

    /// Frame `i` of animation `clip`, the last one past its end; none for an animation the sheet lacks.
    pub fn cell(&self, clip: &str, i: usize) -> Option<Cell> {
        let cells = self.layout.clips.get(clip)?;
        cells.get(i.min(cells.len().saturating_sub(1))).copied()
    }

    /// How many frames animation `clip` has.
    pub fn len(&self, clip: &str) -> usize {
        self.layout.clips.get(clip).map_or(0, Vec::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layout whose every animation is `cell`, on pages `pages` (or a one-page `size`).
    fn layout(pages: &str, cell: &str) -> String {
        let clips: Vec<String> = crate::pick::CLIPS
            .iter()
            .map(|c| format!("\"{c}\":[{cell}]"))
            .collect();
        format!(
            r#"{{{pages},"fps":10,"clips":{{{}}},"portrait":{cell}}}"#,
            clips.join(",")
        )
    }

    #[test]
    fn a_cell_reads_from_its_own_page() -> Result<(), LayoutError> {
        // page 0 is 1x1 red, page 1 is 2x1 blue then green
        let json = layout(
            r#""pages":[[1,1],[2,1]]"#,
            r#"{"page":1,"x":1,"y":0,"w":1,"h":1,"dx":0,"dy":-1}"#,
        );
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255];
        let sheet = Sheet::paged(&json, &rgba)?;
        assert_eq!(sheet.at(0, 0, 0), [255, 0, 0, 255]);
        assert_eq!(sheet.at(1, 1, 0), [0, 255, 0, 255]);
        assert_eq!(sheet.at(2, 0, 0), [0; 4], "no third page");
        assert_eq!(sheet.at(0, 1, 0), [0; 4], "off the first page");
        Ok(())
    }

    #[test]
    fn a_one_page_sheet_may_give_its_size() -> Result<(), LayoutError> {
        let json = layout(
            r#""size":[1,1]"#,
            r#"{"x":0,"y":0,"w":1,"h":1,"dx":0,"dy":-1}"#,
        );
        let sheet = Sheet::new(&json, &[1, 2, 3, 255], 1, 1)?;
        assert_eq!(sheet.layout.pages, vec![[1, 1]]);
        assert_eq!(sheet.at(0, 0, 0), [1, 2, 3, 255]);
        Ok(())
    }

    #[test]
    fn pages_must_hold_the_bytes_and_cells_must_fit_their_page() {
        let json = layout(
            r#""pages":[[1,1],[2,1]]"#,
            r#"{"page":1,"x":1,"y":0,"w":1,"h":1,"dx":0,"dy":-1}"#,
        );
        assert!(matches!(
            Sheet::paged(&json, &[0; 8]),
            Err(LayoutError::Size { .. })
        ));
        let wide = layout(
            r#""pages":[[1,1],[2,1]]"#,
            r#"{"page":0,"x":1,"y":0,"w":1,"h":1,"dx":0,"dy":-1}"#,
        );
        assert!(matches!(
            Sheet::paged(&wide, &[0; 12]),
            Err(LayoutError::Outside { .. })
        ));
        let lost = layout(
            r#""pages":[[1,1]]"#,
            r#"{"page":3,"x":0,"y":0,"w":1,"h":1,"dx":0,"dy":-1}"#,
        );
        assert!(matches!(
            Sheet::paged(&lost, &[0; 4]),
            Err(LayoutError::Outside { .. })
        ));
    }
}
