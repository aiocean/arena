//! A fighter's sprite sheet: one picture or more (pages, each under the 4096 pixels a side a browser takes) holding
//! every frame, and its layout (`<name>.json`, written by `art/cultivator/tools/sheet.rs`): where each animation's
//! frames sit, on which page, and where the feet are in each.

use serde::Deserialize;
use std::collections::BTreeMap;

/// A layout read from its JSON, its frames checked against its pages and the animations `pick` asks for.
pub fn parse(json: &str) -> Result<Layout, LayoutError> {
    let layout: Layout =
        serde_json::from_str(json).map_err(|e| LayoutError::Json(e.to_string()))?;
    for clip in crate::pick::CLIPS {
        if layout.clips.get(*clip).is_none_or(Vec::is_empty) {
            return Err(LayoutError::Missing((*clip).to_string()));
        }
    }
    for (clip, cells) in &layout.clips {
        for (frame, c) in cells.iter().enumerate() {
            let fits = layout
                .pages
                .get(c.page as usize)
                .is_some_and(|[w, h]| c.x + c.w <= *w && c.y + c.h <= *h);
            if !fits {
                return Err(LayoutError::Outside {
                    clip: clip.clone(),
                    frame,
                });
            }
        }
    }
    Ok(layout)
}

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
    pub portrait: Cell,
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
    portrait: Cell,
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

impl Layout {
    /// Frame `i` of animation `clip`, the last one past its end; none for an animation the sheet lacks.
    pub fn cell(&self, clip: &str, i: usize) -> Option<Cell> {
        let cells = self.clips.get(clip)?;
        cells.get(i.min(cells.len().saturating_sub(1))).copied()
    }

    /// How many frames animation `clip` has.
    pub fn len(&self, clip: &str) -> usize {
        self.clips.get(clip).map_or(0, Vec::len)
    }
}
