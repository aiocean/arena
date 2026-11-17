//! The ten kinds of damage, the five phases' cycles, and fusion (`docs/specs/2026-11-17-damage-system-design.md`,
//! section 4). The order of [`Element::ALL`] is the conversion order: damage converts only to a later element.

/// How many kinds of damage there are.
pub const N: usize = 10;

/// An amount per kind of damage, indexed by [`Element::index`].
pub type Parts = [f64; N];

/// A kind of damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Element {
    /// Lực: the body's own force; cut by armour.
    Force,
    /// Kim.
    Metal,
    /// Mộc.
    Wood,
    /// Thủy.
    Water,
    /// Hỏa.
    Fire,
    /// Thổ.
    Earth,
    /// Lôi.
    Thunder,
    /// Dương (light).
    Light,
    /// Âm (death, yin): passes the energy shield.
    Yin,
    /// Hồn (soul): strikes the sense first.
    Soul,
}

impl Element {
    pub const ALL: [Element; N] = [
        Element::Force,
        Element::Metal,
        Element::Wood,
        Element::Water,
        Element::Fire,
        Element::Earth,
        Element::Thunder,
        Element::Light,
        Element::Yin,
        Element::Soul,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Element::Force => "force",
            Element::Metal => "metal",
            Element::Wood => "wood",
            Element::Water => "water",
            Element::Fire => "fire",
            Element::Earth => "earth",
            Element::Thunder => "thunder",
            Element::Light => "light",
            Element::Yin => "yin",
            Element::Soul => "soul",
        }
    }

    /// One of the five phases (Kim, Mộc, Thủy, Hỏa, Thổ).
    pub fn is_phase(self) -> bool {
        matches!(
            self,
            Element::Metal | Element::Wood | Element::Water | Element::Fire | Element::Earth
        )
    }

    /// The phase this one overcomes: Kim khắc Mộc, Mộc khắc Thổ, Thổ khắc Thủy, Thủy khắc Hỏa, Hỏa khắc Kim.
    pub fn overcomes(self) -> Option<Element> {
        match self {
            Element::Metal => Some(Element::Wood),
            Element::Wood => Some(Element::Earth),
            Element::Earth => Some(Element::Water),
            Element::Water => Some(Element::Fire),
            Element::Fire => Some(Element::Metal),
            _ => None,
        }
    }

    /// The phase this one feeds: Kim sinh Thủy, Thủy sinh Mộc, Mộc sinh Hỏa, Hỏa sinh Thổ, Thổ sinh Kim.
    pub fn feeds(self) -> Option<Element> {
        match self {
            Element::Metal => Some(Element::Water),
            Element::Water => Some(Element::Wood),
            Element::Wood => Some(Element::Fire),
            Element::Fire => Some(Element::Earth),
            Element::Earth => Some(Element::Metal),
            _ => None,
        }
    }

    /// The phase that feeds this one.
    pub fn fed_by(self) -> Option<Element> {
        Element::ALL.into_iter().find(|e| e.feeds() == Some(self))
    }

    /// Âm and Dương overcome each other.
    pub fn opposes(self) -> Option<Element> {
        match self {
            Element::Light => Some(Element::Yin),
            Element::Yin => Some(Element::Light),
            _ => None,
        }
    }
}

/// The root (bản mệnh) a fighter may take: a phase, light or yin.
pub fn is_root(e: Element) -> bool {
    e.is_phase() || matches!(e, Element::Light | Element::Yin)
}

/// How a part of element `part` is scaled against a target of root `root` (step 7): overcoming the root 1.25,
/// overcome by it 0.8, the same phase 0.7, light against yin and yin against light 1.5; `dao_fire` (fire of the
/// fourth tier) counts as overcoming a water root. `strong` is the overcome factor (1.25, or 1.5 under Hệ Đại Thành).
pub fn root_factor(part: Element, root: Option<Element>, dao_fire: bool, strong: f64) -> f64 {
    let Some(root) = root else { return 1.0 };
    if dao_fire && part == Element::Fire && root == Element::Water {
        return strong;
    }
    if part.opposes() == Some(root) {
        return ROOT.opposed;
    }
    if part.is_phase() && root.is_phase() {
        if part == root {
            return ROOT.same;
        }
        if part.overcomes() == Some(root) {
            return strong;
        }
        if root.overcomes() == Some(part) {
            return ROOT.overcome;
        }
    }
    1.0
}

/// Step 7's factors against a root.
pub struct Root {
    pub overcomes: f64,
    pub overcome: f64,
    pub same: f64,
    /// The share of a same-phase part the target takes back as qi.
    pub same_qi: f64,
    pub opposed: f64,
}
pub const ROOT: Root = Root {
    overcomes: 1.25,
    overcome: 0.8,
    same: 0.7,
    same_qi: 0.1,
    opposed: 1.5,
};

/// What two kinds of damage make when they land on one target within the fusion window (Tạo).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Fusion {
    /// Phong: fire and water. Pushes 1T and stops qi regen 1 s.
    Wind,
    /// Lôi hỏa: thunder and fire. A second blast 0.3 s later on the sense, half the fused part.
    Stormfire,
    /// Dung nham: earth and fire. A lava zone 2T for 3 s, a tenth of the fused part as fire each second.
    Lava,
    /// Lưỡng nghi: yin and light. The striker takes back a quarter.
    Duality,
}

impl Fusion {
    pub const ALL: [Fusion; 4] = [
        Fusion::Wind,
        Fusion::Stormfire,
        Fusion::Lava,
        Fusion::Duality,
    ];

    /// The pair that makes it; either may come first.
    pub fn pair(self) -> (Element, Element) {
        match self {
            Fusion::Wind => (Element::Fire, Element::Water),
            Fusion::Stormfire => (Element::Thunder, Element::Fire),
            Fusion::Lava => (Element::Earth, Element::Fire),
            Fusion::Duality => (Element::Yin, Element::Light),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Fusion::Wind => "wind",
            Fusion::Stormfire => "stormfire",
            Fusion::Lava => "lava",
            Fusion::Duality => "duality",
        }
    }
}

/// Fusion's numbers.
pub struct Fuse {
    /// The fused part: this share of the smaller of the two.
    pub share: f64,
    /// How long damage stays in the window, in seconds.
    pub window: f64,
    /// Wind: push in T, and qi regen stopped, in seconds.
    pub wind_push: f64,
    pub wind_lock: f64,
    /// Stormfire: the second blast's delay and share, on the sense.
    pub storm_delay: f64,
    pub storm_share: f64,
    /// Lava: the zone's radius in T, its life, and its share a second.
    pub lava_radius: f64,
    pub lava_time: f64,
    pub lava_share: f64,
    /// Duality: the share the striker takes back.
    pub duality_recoil: f64,
}
pub const FUSE: Fuse = Fuse {
    share: 0.4,
    window: 1.0,
    wind_push: 1.0,
    wind_lock: 1.0,
    storm_delay: 0.3,
    storm_share: 0.5,
    lava_radius: 2.0,
    lava_time: 3.0,
    lava_share: 0.1,
    duality_recoil: 0.25,
};
