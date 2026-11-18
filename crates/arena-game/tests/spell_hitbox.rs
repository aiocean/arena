//! The drawn effects against the shapes the rules hit with (`arena_game::audit`, the review page's measure): every
//! skill cast once by each seat at a foe who stands. An effect whose art drifts beyond the audit's tolerances fails,
//! unless it is one of the mismatches already listed for a decision (`KNOWN`: the review page
//! https://arena.aiocean.io/review/spell-hitbox/ shows each). A listed one that comes right fails too, so the list is
//! kept to what is still wrong. Run again after the art changes; regenerate the page with the `spell_hitbox` example.

use arena_game::audit::{Report, Sheets, audit_casts};
use std::path::Path;

/// The effects off today, each awaiting a fix of its art's placement or a decision on the rules' shape.
const KNOWN: &[&str] = &[
    "ambient/rain",
    "arcane/arcane-cast",
    "arcane/arcane-rain",
    "arcane/comma-seal",
    "arcane/gravity-well",
    "arcane/polymorph-poof",
    "arcane/shuriken-seal",
    "arcane/sword-formation",
    "arcane/sword-strike",
    "arcane/twin-shuriken",
    "arcane/vortex-ring",
    "explosions/barrel-explosion",
    "explosions/cluster-bomb",
    "explosions/firework-burst",
    "explosions/implosion",
    "explosions/plasma-explosion",
    "explosions/small-explosion",
    "explosions/swirl-blast",
    "explosions/toxic-explosion",
    "fire/fire-breath",
    "fire/fire-pillar",
    "fire/flame-ring",
    "fire/incinerate",
    "fire/lava-burst",
    "fire/meteor-strike",
    "fire/phoenix-burst",
    "holy/divine-beam",
    "holy/holy-nova",
    "ice/frost-nova",
    "ice/frozen-ground",
    "impacts/blood-splat",
    "impacts/crater-impact",
    "impacts/death-poof",
    "impacts/punch-hit",
    "impacts/shockwave-ring",
    "impacts/star-converge",
    "impacts/twin-converge",
    "lightning/arc-discharge",
    "lightning/chain-lightning",
    "lightning/lightning-cast",
    "lightning/lightning-strike",
    "lightning/lightning-whip",
    "lightning/static-burst",
    "lightning/storm-field",
    "melee/axe-smear",
    "melee/hammer-smash",
    "melee/quickdraw-slash",
    "melee/scythe-reap",
    "melee/shadow-claw",
    "melee/shield-bash",
    "melee/spin-slash",
    "melee/sword-combo",
    "melee/uppercut",
    "nature/earth-spike",
    "nature/fissure",
    "nature/nature-cast",
    "nature/poison-cloud",
    "nature/rockfall",
    "nature/spore-burst",
    "nature/thorn-wall",
    "projectiles/arrow-flight",
    "scifi/emp-pulse",
    "scifi/mech-explosion",
    "scifi/railgun-shot",
    "shadow/curse",
    "shadow/shadow-spikes",
    "shadow/shadow-tendrils",
    "shadow/winter-ordeal",
    "status/petrified",
    "status/silenced",
    "ui/debuff-arrow",
    "water/water-jet",
    "water/whirlpool",
    "wind/sonic-boom",
    "wind/wind-gust",
];

fn sheets() -> Result<Sheets, String> {
    Sheets::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("assets"))
}

fn casts(ids: &[&'static str]) -> Result<Vec<Report>, String> {
    let mut s = sheets()?;
    Ok(audit_casts(&mut s, ids))
}

/// The orb's art is drawn at the ball the rules test as it wanders round the origin the rules move (`shot::orb_at`;
/// the sheet's own frames carry the loop): their centres within under half the ball's radius, all its flight (the
/// sheet steps at its frame rate, the rules every tick).
#[test]
fn the_orb_is_drawn_where_it_hits() -> Result<(), String> {
    let reports = casts(&["orb"])?;
    let orb = reports
        .iter()
        .find(|r| r.spells.iter().any(|s| s == "orb") && r.marks.iter().any(|m| m == "shot"))
        .ok_or("no orb effect measured against its ball")?;
    let offset = orb
        .offset
        .ok_or("the orb's art and ball never seen together")?;
    assert!(
        offset <= 0.45,
        "{}: the art's centre {offset:.2} half-sizes from the ball's ({:?})",
        orb.fx,
        orb.verdict
    );
    Ok(())
}

#[test]
fn no_drawn_effect_drifts_from_its_hit_shape() -> Result<(), String> {
    let ids: Vec<&'static str> = arena_duel::data::spells().iter().map(|s| s.name).collect();
    let reports = casts(&ids)?;
    let measured: Vec<&Report> = reports
        .iter()
        .filter(|r| !r.decoration() && !r.unplayed)
        .collect();
    assert!(
        measured.len() > 20,
        "only {} effects measured",
        measured.len()
    );
    let new: Vec<String> = measured
        .iter()
        .filter(|r| !r.ok() && !KNOWN.contains(&r.fx.as_str()))
        .map(|r| format!("{}: {}", r.fx, r.verdict.join("; ")))
        .collect();
    let fixed: Vec<&str> = KNOWN
        .iter()
        .filter(|k| measured.iter().any(|r| r.fx == **k && r.ok()))
        .copied()
        .collect();
    assert!(
        new.is_empty(),
        "drawn effects off their hit shapes:\n{}",
        new.join("\n")
    );
    assert!(
        fixed.is_empty(),
        "now right, take them off KNOWN: {fixed:?}"
    );
    Ok(())
}
