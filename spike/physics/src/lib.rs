//! Physics spike: the arena's hand-written collision queries against parry2d-f64, rapier2d-f64 and avian2d (f64),
//! on the geometry of real duels (seeds 1..=S, 120 s each, brains), plus synthetic probes on the integer grid.

use arena_duel::duel::Duel;
use arena_duel::fighter::State;
use arena_duel::fingerprint::{fnv1a, landing};
use arena_duel::shot::ShotKind;

#[cfg(feature = "avian")]
pub mod avian;
pub mod hand;
#[cfg(feature = "parry")]
pub mod parry;
#[cfg(feature = "rapier32")]
pub mod parry32;
#[cfg(feature = "rapier")]
pub mod rapier;

const T: f64 = arena_duel::data::T;

/// A bolt query: from `p`, along `d` (length 1), `reach` T, among `targets` (id, x, y, r in T): the first entered.
#[derive(Clone, Debug)]
pub struct BoltQ {
    pub px: f64,
    pub py: f64,
    pub dx: f64,
    pub dy: f64,
    pub reach: f64,
    pub owner: usize,
}

/// One tick's geometry and queries.
#[derive(Clone, Debug, Default)]
pub struct Geo {
    /// Each fighter's hurtboxes (arena px, y up), as `frames::hurtboxes`.
    pub hurt: [Vec<[f64; 4]>; 2],
    /// Each fighter's bolt targets (id, x, y, r) in T, as `unit::targets_of` lays the foe's circles.
    pub circles: [Vec<(u32, f64, f64, f64)>; 2],
    /// Each fighter's strike box (`unit::strike_box`, blade), tested against the foe's hurtboxes.
    pub strike: [[f64; 4]; 2],
    /// Point-with-margin probes: (foe, x, y, r), as `shot::overlaps`.
    pub probes: Vec<(usize, f64, f64, f64)>,
    pub bolts: Vec<BoltQ>,
}

pub trait Backend {
    fn load(&mut self, g: &Geo);
    fn strike(&mut self, g: &Geo, me: usize) -> bool;
    fn probe(&mut self, g: &Geo, i: usize) -> bool;
    /// The first target entered: (id, T along the ray).
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)>;
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn int(&mut self, lo: i64, hi: i64) -> f64 {
        (lo + (self.next() % ((hi - lo) as u64)) as i64) as f64
    }
}

fn geo_of(state: &State, rng: &mut Lcg) -> Geo {
    let mut g = Geo::default();
    for f in 0..2 {
        g.hurt[f] = arena_duel::frames::hurtboxes(state, f);
        let alive = state.fighters[f].alive() && !state.fighters[f].body.untouchable();
        if alive {
            for h in &g.hurt[f] {
                for (cx, cy, r) in arena_duel::unit::circles(*h) {
                    g.circles[f].push((f as u32, state.to_t(cx), cy / T, r / T));
                }
            }
        }
        let b = &state.fighters[f].body;
        g.strike[f] =
            arena_duel::unit::strike_box(state, f, b.x, b.air, b.face, 90.0, b.spec.body, true);
    }
    for s in &state.shots {
        let foe = 1 - s.owner;
        let r = s.spell.radius.unwrap_or(0.0);
        match &s.kind {
            ShotKind::Bolt { flight } => {
                let (x, y) = (state.from_t(flight.pos.x), flight.pos.y * T);
                g.probes.push((foe, x, y, 36.0));
                g.probes.push((foe, x, y, r));
                g.bolts.push(BoltQ {
                    px: flight.pos.x,
                    py: flight.pos.y,
                    dx: flight.dir.x,
                    dy: flight.dir.y,
                    reach: flight.speed / 60.0,
                    owner: s.owner,
                });
            }
            ShotKind::Orb { .. } => {
                let (bx, by) = arena_duel::shot::orb_at(s);
                g.probes.push((foe, bx, -by, r));
            }
            _ => {}
        }
    }
    // synthetic probes on the integer and half-integer grid around each fighter: the boxes' edges are whole pixels,
    // so these land on edges often (touching is where engines and the rules may disagree)
    for k in 0..16 {
        let foe = k % 2;
        let fx = state.fighters[foe].body.x;
        let frac = [0.0, 0.5, 0.25, 0.1][(rng.next() % 4) as usize];
        let x = fx + rng.int(-120, 120) + frac;
        let y = state.fighters[foe].body.air + rng.int(-10, 260);
        let r = [0.0, 4.0, 12.5, 36.0][(rng.next() % 4) as usize];
        g.probes.push((foe, x, y, r));
    }
    // synthetic bolts from the other fighter's chest toward the foe, a little up or down
    for k in 0..4 {
        let owner = k % 2;
        let me = &state.fighters[owner].body;
        let foe = &state.fighters[1 - owner].body;
        let (ox, oy) = (state.to_t(me.x), (me.air + rng.int(40, 200)) / T);
        let dir = if foe.x >= me.x { 1.0 } else { -1.0 };
        let (dx, dy) = (dir, rng.int(-30, 30) / 100.0);
        let n = libm::sqrt(dx * dx + dy * dy);
        g.bolts.push(BoltQ {
            px: ox,
            py: oy,
            dx: dx / n,
            dy: dy / n,
            reach: rng.int(1, 40) / 4.0,
            owner,
        });
    }
    g
}

/// Every tick's geometry of seeds `1..=seeds`, `seconds` each.
pub fn scenes(seeds: u32, seconds: f64) -> Vec<Geo> {
    let mut out = Vec::new();
    let mut rng = Lcg(7);
    for seed in 1..=seeds {
        let mut duel = Duel::with_brains(landing(seed));
        for _ in 0..(seconds * 60.0) as usize {
            duel.step(1.0 / 60.0);
            out.push(geo_of(duel.state(), &mut rng));
        }
    }
    out
}

/// Answers of one backend over `scenes`, as text lines (one per query).
pub fn answers(b: &mut dyn Backend, scenes: &[Geo]) -> Vec<String> {
    let mut out = Vec::new();
    for g in scenes {
        b.load(g);
        for me in 0..2 {
            out.push(format!("s{}", b.strike(g, me) as u8));
        }
        for i in 0..g.probes.len() {
            out.push(format!("p{}", b.probe(g, i) as u8));
        }
        for i in 0..g.bolts.len() {
            out.push(match b.bolt(g, i) {
                Some((id, t)) => format!("b{id}:{:016x}", t.to_bits()),
                None => "b-".into(),
            });
        }
    }
    out
}

pub fn hash(lines: &[String]) -> u64 {
    fnv1a(lines.join("\n").as_bytes())
}

/// Counts of mismatches against the hand-written answers, by kind (strike, probe, bolt hit/miss, bolt same id but
/// other bits).
pub fn diff(base: &[String], other: &[String]) -> String {
    let (mut s, mut p, mut b, mut bt) = (0, 0, 0, 0);
    let mut first = Vec::new();
    for (i, (a, o)) in base.iter().zip(other).enumerate() {
        if a == o {
            continue;
        }
        match &a[..1] {
            "s" => s += 1,
            "p" => p += 1,
            _ => {
                if a.split(':').next() == o.split(':').next() {
                    bt += 1
                } else {
                    b += 1
                }
            }
        }
        if first.len() < 3 {
            first.push(format!("#{i}: {a} vs {o}"));
        }
    }
    format!(
        "strike {s}, probe {p}, bolt target {b}, bolt toi bits {bt} of {} ({})",
        base.len(),
        first.join("; ")
    )
}

pub fn backends() -> Vec<(&'static str, Box<dyn Backend>)> {
    #[allow(unused_mut)]
    let mut v: Vec<(&'static str, Box<dyn Backend>)> = vec![("hand", Box::new(hand::Hand))];
    #[cfg(feature = "parry")]
    v.push(("parry2d-f64", Box::new(parry::Parry)));
    #[cfg(feature = "parry")]
    v.push(("parry2d-f64 strict", Box::new(parry::ParryStrict)));
    #[cfg(feature = "rapier")]
    v.push(("rapier2d-f64", Box::new(rapier::Rapier::new())));
    #[cfg(feature = "rapier32")]
    v.push((
        "parry2d-f32 (rapier2d, bevy_rapier2d)",
        Box::new(parry32::Parry32),
    ));
    #[cfg(feature = "avian")]
    v.push(("avian2d-f64", Box::new(avian::Avian::new())));
    v
}

/// The report: each backend's hash over all answers, and its mismatches against the hand-written ones.
pub fn report(seeds: u32, seconds: f64) -> String {
    let t0 = now();
    let scenes = scenes(seeds, seconds);
    let duel_ns = (now() - t0) / scenes.len() as f64;
    let q: usize = scenes
        .iter()
        .map(|g| 2 + g.probes.len() + g.bolts.len())
        .sum();
    let mut out = format!(
        "scenes {} queries {} (duel step + geometry {:.0} ns/tick)\n",
        scenes.len(),
        q,
        duel_ns
    );
    let mut base = None;
    for (name, mut b) in backends() {
        let t = now();
        let a = answers(b.as_mut(), &scenes);
        let ns = (now() - t) / scenes.len() as f64;
        let h = hash(&a);
        let d = base
            .as_ref()
            .map(|base: &Vec<String>| diff(base, &a))
            .unwrap_or_default();
        out.push_str(&format!("{name}: {h:016x} {ns:.0} ns/tick {d}\n"));
        if base.is_none() {
            base = Some(a);
        }
    }
    out
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_nanos() as f64
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = performance, js_name = now)]
    fn now_ms() -> f64;
}
#[cfg(target_arch = "wasm32")]
fn now() -> f64 {
    now_ms() * 1e6
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn wasm_report(seeds: u32, seconds: f64) -> String {
    report(seeds, seconds)
}
