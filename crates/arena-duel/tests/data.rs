//! The duel's tables, dice and maths, pinned against `tests/fixtures/js_numbers.json`: the motions skills borrow, the
//! looks, the brain's one temperament and the constants. The skills' damage, costs and timings are `arena-calc`'s
//! (`crates/arena-calc/tests/gem.rs` pins them); here, that each skill the duel casts is its gem on its motion.

use arena_mind::Rng;
use arena_duel::data::{self, Kind, Look, block_chance, orb_offset};
use serde_json::{Map, Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/js_numbers.json")).unwrap_or(Value::Null)
}

/// Every number as an f64, so 24 and 24.0 compare equal.
fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap_or(f64::NAN)),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), norm(v))).collect()),
        other => other.clone(),
    }
}

fn num(v: &Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

#[test]
fn rng_matches_js_for_edge_seeds() -> Result<(), String> {
    let f = fixture();
    let seeds = f["rng"]
        .as_object()
        .ok_or_else(|| "fixture has no rng".to_string())?;
    assert_eq!(seeds.len(), 6);
    for (seed, draws) in seeds {
        let mut rng = Rng::new(seed.parse::<u64>().unwrap_or(0) as u32);
        for (i, d) in draws.as_array().into_iter().flatten().enumerate() {
            assert_eq!(rng.next(), num(d), "seed {seed} draw {i}");
        }
    }
    Ok(())
}

#[test]
fn orb_offset_matches_js() {
    for row in fixture()["orbOffset"].as_array().into_iter().flatten() {
        let (x, y) = orb_offset(num(&row[0]));
        assert!(
            (x - num(&row[1])).abs() < 1e-12 && (y - num(&row[2])).abs() < 1e-12,
            "t {}",
            row[0]
        );
    }
}

#[test]
fn block_chance_matches_js() {
    for row in fixture()["blockChance"].as_array().into_iter().flatten() {
        assert_eq!(block_chance(num(&row[0])), num(&row[1]), "power {}", row[0]);
    }
}

fn kind_name(k: Kind) -> &'static str {
    k.name()
}

/// A spell in the fixture's shape: fields left out when they are absent or false.
fn spell_json(s: &data::Spell) -> Value {
    let mut o = Map::new();
    let mut put = |k: &str, v: Value| {
        o.insert(k.to_string(), v);
    };
    put("kind", json!(kind_name(s.kind)));
    for (k, v) in [
        ("fx", s.fx),
        ("hitFx", s.hit_fx),
        ("endFx", s.end_fx),
        ("slamFx", s.slam_fx),
    ] {
        if let Some(v) = v {
            put(k, json!(v));
        }
    }
    for (k, v) in [
        ("cost", s.cost),
        ("windup", s.windup),
        ("recover", s.recover),
        ("cooldown", s.cooldown),
    ] {
        put(k, json!(v));
    }
    for (k, v) in [
        ("range", s.range),
        ("distance", s.distance),
        ("damage", s.damage),
        ("sigil", s.sigil),
        ("impact", s.impact),
        ("radius", s.radius),
        ("notice", s.notice),
        ("life", s.life),
        ("height", s.height),
        ("pull", s.pull),
        ("reach", s.reach),
        ("under", s.under),
        ("tick", s.tick),
        ("speed", s.speed),
        ("pullUntil", s.pull_until),
        ("strikes", s.strikes),
        ("every", s.every),
        ("aimAhead", s.aim_ahead),
        ("amount", s.amount),
        ("time", s.time),
        ("spray", s.spray),
    ] {
        if let Some(v) = v {
            put(k, json!(v));
        }
    }
    for (k, v) in [
        ("dive", s.dive),
        ("grounded", s.grounded),
        ("ultimate", s.ultimate),
        ("reflect", s.reflect),
        ("feet", s.feet),
        ("swap", s.swap),
    ] {
        if v {
            put(k, json!(true));
        }
    }
    if let Some(c) = s.counter {
        put("counter", json!({ "damage": c.damage, "stun": c.stun }));
    }
    Value::Object(o)
}

#[test]
fn every_motion_is_ported_field_for_field() -> Result<(), String> {
    let f = fixture();
    let js = f["motions"]
        .as_object()
        .ok_or_else(|| "fixture has no motions".to_string())?;
    let names: Vec<&str> = data::motions().iter().map(|s| s.name).collect();
    assert_eq!(
        names,
        js.keys().map(String::as_str).collect::<Vec<_>>(),
        "names in the fixture's order"
    );
    for (name, want) in js {
        let s = data::motion(name).ok_or_else(|| format!("no motion {name}"))?;
        let got = spell_json(s);
        // keys in any order: compare as sorted maps
        let sort = |v: &Value| {
            v.as_object().map(|o| {
                o.iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
        };
        assert_eq!(sort(&norm(&got)), sort(&norm(want)), "motion {name}");
    }
    Ok(())
}

#[test]
fn no_frost_skill_is_left() {
    for gone in [
        "icicle",
        "shard",
        "shatter",
        "frostfire",
        "flurry",
        "prison",
    ] {
        assert!(data::motion(gone).is_none(), "{gone}");
        assert!(data::spell(gone).is_none(), "{gone}");
    }
}

#[test]
fn every_skill_is_its_gem_on_its_motion() {
    use arena_calc::gem::{GEMS, Resource};
    for g in GEMS {
        let m = data::motion(g.spell);
        assert!(
            m.is_some(),
            "{} borrows {}, which the duel has",
            g.id,
            g.spell
        );
        let (Some(m), Some(s)) = (m, data::spell(g.id)) else {
            continue;
        };
        let qi: f64 = g
            .costs
            .iter()
            .filter(|c| c.0 == Resource::Qi)
            .map(|c| c.1)
            .sum();
        assert_eq!(s.name, g.id);
        assert_eq!(
            (s.kind, s.fx, s.range, s.speed),
            (m.kind, m.fx, m.range, m.speed),
            "{}",
            g.id
        );
        assert_eq!(
            (s.windup, s.recover, s.cooldown, s.cost),
            (g.windup, g.recover, g.cooldown, qi),
            "{}",
            g.id
        );
        assert_eq!(s.ultimate, g.is_ultimate(), "{}", g.id);
        assert_eq!(s.damage.is_some(), g.deals_damage(), "{}", g.id);
    }
    assert_eq!(data::spells().len(), GEMS.len());
}

#[test]
fn each_look_is_ported() -> Result<(), String> {
    let f = fixture();
    for (key, l) in [("thunder", Look::Thunder), ("frost", Look::Frost)] {
        let d = data::look(l);
        let js = &f["looks"][key];
        assert_eq!(json!(d.name), js["name"]);
        assert_eq!(json!(d.bolt), js["bolt"]);
        assert_eq!(json!(d.circle), js["circle"]);
        let tints = js["tint"]
            .as_object()
            .ok_or_else(|| format!("{key} has no tint"))?;
        assert_eq!(d.tint.len(), tints.len(), "{key} tints");
        for (fx, want) in tints {
            let (_, t) = d
                .tint
                .iter()
                .find(|(name, _)| name == fx)
                .ok_or_else(|| format!("{key} tint {fx}"))?;
            let mut o = Map::new();
            for (k, v) in [
                ("palette", t.palette),
                ("accent", t.accent),
                ("stars", t.stars),
                ("fire", t.fire),
            ] {
                if let Some(v) = v {
                    o.insert(k.into(), json!(v));
                }
            }
            if let Some(g) = t.glow {
                o.insert("glow".into(), json!(g));
            }
            for (k, v) in [("width", t.width), ("height", t.height)] {
                if let Some(v) = v {
                    o.insert(k.into(), json!(v));
                }
            }
            assert_eq!(norm(&Value::Object(o)), norm(want), "{key} tint {fx}");
        }
    }
    let m = data::MIND;
    let got = json!({
        "postures": m.postures, "range": m.range, "dry": m.dry, "aggression": m.aggression, "tempo": m.tempo,
        "sky": m.sky,
    });
    assert_eq!(norm(&got), norm(&f["mind"]));
    Ok(())
}

#[test]
fn the_constants_are_ported() {
    let f = fixture();
    assert_eq!(data::RAGE, num(&f["constants"]["RAGE"]));
    assert_eq!(data::STALE, num(&f["constants"]["STALE"]));
    assert_eq!(data::POOL_SIZE as f64, num(&f["constants"]["POOL_SIZE"]));
    assert_eq!(
        f64::from(data::SKILL_LEVEL),
        num(&f["constants"]["SKILL_LEVEL"])
    );
    assert_eq!(data::T, num(&f["constants"]["T"]));
    let fr = data::FRAMES;
    let frames = json!({
        "counter": { "power": fr.counter.power, "stagger": fr.counter.stagger },
        "punish": { "power": fr.punish.power, "stagger": fr.punish.stagger },
        "blockedRecover": fr.blocked_recover,
        "unsafeOnBlock": fr.unsafe_on_block,
        "ultimateInvincible": fr.ultimate_invincible,
    });
    assert_eq!(norm(&frames), norm(&f["constants"]["FRAMES"]));
    assert_eq!(
        json!({ "lamp": data::DECOR[0], "night": data::DECOR[1] }),
        f["constants"]["DECOR"]
    );
}
