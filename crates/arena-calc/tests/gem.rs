//! The shared pool (`calc::gem::GEMS`): every skill well formed, every tag group covered, nothing of ice left, and
//! every borrowed motion a real spell of the duel (`tests/fixtures/spells.json`: the duel's `data::SPELLS` when the
//! pool was made, its numbers the pool's bases and timings came from).

use arena_calc::ailment::ForceAilment;
use arena_calc::aura::AuraKind;
use arena_calc::curse::Kind;
use arena_calc::domain::DomainKind;
use arena_calc::element::Element;
use arena_calc::gem::{
    self, GEMS, Gem, Resource, TODAY_ATK, TODAY_DEFENCE, Tier, aura_of, band, curse_of, domain_of,
    power_score, safety_factor, starting_tier,
};
use arena_calc::hit::spell_base;
use arena_calc::tag::{Group, Tag};
use std::collections::HashSet;

/// The ice skills the classless duel removes (design, 0 and 11.8).
/// A spell of the duel as `data::SPELLS` had it.
struct Spell {
    name: String,
    damage: Option<f64>,
    windup: f64,
    recover: f64,
    cooldown: f64,
    cost: f64,
}

fn spells() -> Vec<Spell> {
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/spells.json")).unwrap_or_default();
    rows.iter()
        .map(|r| Spell {
            name: r["name"].as_str().unwrap_or_default().to_string(),
            damage: r["damage"].as_f64(),
            windup: r["windup"].as_f64().unwrap_or_default(),
            recover: r["recover"].as_f64().unwrap_or_default(),
            cooldown: r["cooldown"].as_f64().unwrap_or_default(),
            cost: r["cost"].as_f64().unwrap_or_default(),
        })
        .collect()
}

fn spell(name: &str) -> Option<Spell> {
    spells().into_iter().find(|s| s.name == name)
}

const REMOVED: [&str; 6] = [
    "icicle",
    "shard",
    "shatter",
    "frostfire",
    "flurry",
    "prison",
];
/// Words that may not appear in any id, name, tag or borrowed spell.
const ICE_WORDS: [&str; 4] = ["frost", "ice", "freeze", "chill"];

fn in_group(g: &Gem, group: Group) -> Vec<Tag> {
    g.tags
        .iter()
        .copied()
        .filter(|t| t.group() == group)
        .collect()
}

#[test]
fn ids_are_unique_and_found() {
    let mut seen = HashSet::new();
    for g in GEMS {
        assert!(seen.insert(g.id), "{} twice", g.id);
        assert!(
            g.id.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
            "{}: ids are short lowercase ascii",
            g.id
        );
        assert_eq!(gem::gem(g.id).map(|x| x.id), Some(g.id));
        assert!(!g.name.is_empty(), "{} has no name", g.id);
    }
    assert!(gem::gem("nothing").is_none());
}

#[test]
fn splits_sum_to_one_or_are_empty_for_skills_that_deal_none() {
    for g in GEMS {
        if g.split.is_empty() {
            assert_eq!(g.base, 0.0, "{}: a base and no split", g.id);
            assert!(
                g.tags().has(Tag::Body) || g.tags().has(Tag::Curse),
                "{}: only body skills and pure curses deal no damage",
                g.id
            );
            continue;
        }
        let sum: f64 = g.split.iter().map(|(_, s)| s).sum();
        assert!((sum - 1.0).abs() <= 1e-9, "{}: split sums to {sum}", g.id);
        assert!(g.split.iter().all(|(_, s)| *s > 0.0), "{}", g.id);
        let kinds: HashSet<Element> = g.split.iter().map(|(e, _)| *e).collect();
        assert_eq!(kinds.len(), g.split.len(), "{}: a kind twice", g.id);
        assert!(g.base > 0.0, "{}: a split and no base", g.id);
    }
}

#[test]
fn body_skills_deal_no_damage() {
    for g in GEMS.iter().filter(|g| g.tags().has(Tag::Body)) {
        assert!(g.split.is_empty() && g.base == 0.0, "{}", g.id);
    }
}

#[test]
fn exactly_one_nature_and_one_reach_at_most_one_role() {
    for g in GEMS {
        assert_eq!(in_group(g, Group::Nature).len(), 1, "{}: nature", g.id);
        assert_eq!(in_group(g, Group::Reach).len(), 1, "{}: reach", g.id);
        assert!(in_group(g, Group::Role).len() <= 1, "{}: roles", g.id);
        assert!(
            in_group(g, Group::Damage).is_empty(),
            "{}: kinds of damage come from the split",
            g.id
        );
        let unique: HashSet<Tag> = g.tags.iter().copied().collect();
        assert_eq!(unique.len(), g.tags.len(), "{}: a tag twice", g.id);
    }
}

#[test]
fn every_borrowed_spell_exists_and_none_is_ice() {
    for g in GEMS {
        assert!(
            spell(g.spell).is_some(),
            "{} borrows {}, which data.rs does not have",
            g.id,
            g.spell
        );
        assert!(!REMOVED.contains(&g.spell), "{} borrows {}", g.id, g.spell);
        assert!(!REMOVED.contains(&g.id), "{} is a removed ice skill", g.id);
    }
}

#[test]
fn nothing_of_ice_anywhere() {
    for g in GEMS {
        let mut words = vec![g.id.to_string(), g.name.to_lowercase(), g.spell.to_string()];
        words.extend(g.tags.iter().map(|t| t.name().to_string()));
        for w in &words {
            for ice in ICE_WORDS {
                assert!(!w.contains(ice), "{}: '{w}' has '{ice}'", g.id);
            }
        }
    }
    for t in Tag::ALL {
        for ice in ICE_WORDS {
            assert!(!t.name().contains(ice), "tag {}", t.name());
        }
    }
}

#[test]
fn every_delivery_and_role_tag_has_a_skill() {
    for t in Tag::ALL.iter().filter(|t| {
        matches!(
            t.group(),
            Group::Delivery | Group::Role | Group::Nature | Group::Reach
        )
    }) {
        assert!(
            GEMS.iter().any(|g| g.tags().has(*t)),
            "no skill is {}",
            t.name()
        );
    }
}

#[test]
fn the_pool_has_ultimates_and_every_kind_of_damage() {
    let ultimates: Vec<&str> = GEMS
        .iter()
        .filter(|g| g.is_ultimate())
        .map(|g| g.id)
        .collect();
    assert!(!ultimates.is_empty());
    for id in ["tribulation", "remnantdusk", "sukiep", "daofusion"] {
        assert!(ultimates.contains(&id), "{id} is an ultimate");
    }
    for g in GEMS.iter().filter(|g| g.is_ultimate()) {
        assert!(
            g.costs.is_empty() && g.cooldown == 0.0,
            "{}: an ultimate is paid with rage",
            g.id
        );
    }
    for e in Element::ALL {
        assert!(
            GEMS.iter().any(|g| g.split.iter().any(|(k, _)| *k == e)),
            "no skill deals {}",
            e.name()
        );
    }
}

#[test]
fn the_design_pool_is_all_here() {
    // Section 11, every row (basics, 11.2-11.8), and the research techniques it leaves out.
    let design = "slash dive blink step palm burst ancientgod slashluo onestep nineturn vermilion heavenlychop \
        fireescape warprint sevencolor devour hundredfist tenfist manshen devourbody autumn manspear godquake bolt orb \
        spark fireball finger jithunder demonfinger tigerflag celestialslaughter spiritchange thirdeye seize voidnail \
        springsoul jet extremesense bodyfix silentfinger fivearrows libow godfinger slaughterbolt summonwind seal \
        collapse quake gravity earthsplit gather mountain ancientthunder fingerdeath flameorigin eightextremes yinmoon \
        restriction plumblossom lifedeath remnantnight heavenflip umbrella starfall timerestrict servitude soulvortex \
        starrotation truefalse spiritsummon threerestrict movemountain karmaseal void arsenal summonrain beans \
        lifedeathrestrict soulbanner ghostsummon daoyellow wanderingsouls restrictbanner yellowfinger moonwing \
        heartbeat kunjiwhip soulsearch dreamdao yellowspring starsoul hex strawknot ancestorcurse exposemark ward bell \
        mirror lightshield ancientprotect karma dreamback meritspirit heal indestructible flowingmoon soulrefine \
        bodyformation godform fate inchearth earthescape ancientcauldron starcompass teleport slaughteraura timeaura \
        tribulation remnantdusk sukiep daofusion lotus";
    let research = "banspear thunderchart starbreak defiance warseal fourforms soulmerge soulburst bloodpowder \
        threeslay liphong timereverse cursewords fourseasons realfake heaventhread bannertribulation manhair skyqueen \
        soularmour deathlessbreath";
    for id in design.split_whitespace().chain(research.split_whitespace()) {
        assert!(gem::gem(id).is_some(), "{id} is missing");
    }
    // Every data.rs spell but the ice ones is in the pool under its own name.
    let all = spells();
    assert!(all.len() > 80, "the spells fixture is read");
    for s in &all {
        if !REMOVED.contains(&s.name.as_str()) {
            assert!(gem::gem(&s.name).is_some(), "{} is missing", s.name);
        }
    }
}

#[test]
fn bases_convert_todays_power() {
    let per_power = TODAY_ATK / 10.0 * TODAY_DEFENCE;
    for g in GEMS.iter().filter(|g| g.spell == g.id) {
        let Some(power) = spell(g.id).and_then(|s| s.damage) else {
            continue;
        };
        if g.is_attack() {
            assert!(
                (g.base - power / 10.0).abs() < 1e-9,
                "{}: effectiveness {} for power {power}",
                g.id,
                g.base
            );
        } else {
            let today = power * per_power;
            let cast = spell_base(g.base, gem::BASE_LEVEL);
            assert!(
                (cast / today - 1.0).abs() < 1e-9,
                "{}: a level-10 cast deals {cast}, today {today}",
                g.id
            );
        }
    }
}

#[test]
fn timings_and_costs_are_sane() {
    for g in GEMS {
        assert!(
            g.windup >= 0.0 && g.recover >= 0.0 && g.cooldown >= 0.0,
            "{}",
            g.id
        );
        assert!(g.windup + g.recover > 0.0, "{}", g.id);
        assert!(g.costs.iter().all(|(_, c)| *c > 0.0), "{}", g.id);
        if let Some(s) = spell(g.id) {
            // a data.rs skill keeps its numbers (ultimates are paid with rage, so their qi is gone)
            assert_eq!(
                (g.windup, g.recover),
                (s.windup, s.recover),
                "{}: timings",
                g.id
            );
            if !g.is_ultimate() {
                assert_eq!(g.cooldown, s.cooldown, "{}: cooldown", g.id);
                let qi = g
                    .costs
                    .iter()
                    .find(|(r, _)| *r == Resource::Qi)
                    .map_or(0.0, |(_, c)| *c);
                assert_eq!(qi, s.cost, "{}: qi", g.id);
            }
        }
    }
}

#[test]
fn one_use_cards_and_restrictions() {
    for g in GEMS {
        let vaal = g.tags().has(Tag::Vaal);
        assert_eq!(g.uses.is_some(), vaal, "{}: a one-use card is Vaal", g.id);
        assert_eq!(g.bluff, vaal, "{}: a one-use card can be shown", g.id);
        assert!(g.uses.is_none_or(|n| n > 0), "{}", g.id);
        assert_eq!(
            g.restriction,
            g.tags().has(Tag::Restriction),
            "{}: a cấm chế stands outside the ladder",
            g.id
        );
    }
    assert_eq!(gem::gem("libow").and_then(|g| g.uses), Some(3));
    assert_eq!(gem::gem("bannertribulation").and_then(|g| g.uses), Some(1));
    assert_eq!(gem::gem("heaventhread").and_then(|g| g.uses), Some(1));
}

#[test]
fn force_shakes_from_fists_palms_and_slams() {
    for g in GEMS {
        let shakes = g.tags().any(&[Tag::Fist, Tag::Palm, Tag::Slam]);
        let want = if shakes {
            ForceAilment::Daze
        } else {
            ForceAilment::Bleed
        };
        assert_eq!(g.force, want, "{}", g.id);
    }
}

#[test]
fn starting_tiers_follow_the_budget() {
    let mut count = [0usize; 4];
    for g in GEMS {
        // measurement (design 12.3) may move a tier; it starts at the budget's and stays within a step of it
        let gap = (g.tier as i32 - starting_tier(g) as i32).abs();
        assert!(
            gap <= 1,
            "{}: {:?}, budget {:?}",
            g.id,
            g.tier,
            starting_tier(g)
        );
        count[g.tier as usize] += 1;
    }
    assert!(count.iter().all(|&n| n > 0), "{count:?}");
    assert_eq!(band(0.0), Tier::C);
    assert_eq!(band(f64::INFINITY), Tier::S);
    // a skill's score grows with its damage
    let (Some(spark), Some(gather)) = (gem::gem("spark"), gem::gem("gather")) else {
        return;
    };
    assert!(power_score(spark) > 0.0 && power_score(gather) > 0.0);
    assert_eq!(gem::gem("heal").map(power_score), Some(0.0));
}

/// Every curse kind, one of each (the kinds that carry a choice with any value: `Kind::index` ignores it). The match
/// makes a new kind fail to compile here until it is listed.
fn every_curse() -> Vec<Kind> {
    let all = [
        Kind::StrawKnot,
        Kind::NineDeaths,
        Kind::AncestorDoom,
        Kind::DeathSeal,
        Kind::FiveOrgans,
        Kind::ThreeBreaths,
        Kind::BindingWord(Tag::Guard),
        Kind::DreamDao,
        Kind::SealGod(Tag::Fire),
        Kind::Demonize,
        Kind::Aging,
        Kind::ExposeMark(Element::Fire),
    ];
    for k in all {
        match k {
            Kind::StrawKnot
            | Kind::NineDeaths
            | Kind::AncestorDoom
            | Kind::DeathSeal
            | Kind::FiveOrgans
            | Kind::ThreeBreaths
            | Kind::BindingWord(_)
            | Kind::DreamDao
            | Kind::SealGod(_)
            | Kind::Demonize
            | Kind::Aging
            | Kind::ExposeMark(_) => {}
        }
    }
    all.to_vec()
}

#[test]
fn every_curse_domain_and_aura_has_a_gem() {
    for k in every_curse() {
        let by: Vec<&str> = GEMS
            .iter()
            .filter(|g| curse_of(g).is_some_and(|c| c.index() == k.index()))
            .map(|g| g.id)
            .collect();
        assert!(!by.is_empty(), "no gem lays {}", k.name());
    }
    for d in DomainKind::ALL {
        assert!(
            GEMS.iter().any(|g| domain_of(g) == Some(d)),
            "no gem raises {d:?}"
        );
    }
    for a in AuraKind::ALL {
        assert!(
            GEMS.iter().any(|g| aura_of(g) == Some(a)),
            "no gem lights {a:?}"
        );
    }
    // and the helpers name only gems that are what they say
    for g in GEMS {
        if curse_of(g).is_some() && g.id != "demonfinger" {
            assert!(g.tags().all(&[Tag::Curse, Tag::Hex]), "{}", g.id);
        }
        if domain_of(g).is_some() {
            assert!(g.tags().has(Tag::Domain), "{}", g.id);
        }
        if aura_of(g).is_some() {
            assert!(g.tags().all(&[Tag::Aura, Tag::Intent]), "{}", g.id);
        }
    }
}

#[test]
fn safer_skills_score_higher() {
    let (Some(thirdeye), Some(spark)) = (gem::gem("thirdeye"), gem::gem("spark")) else {
        return;
    };
    assert!(safety_factor(thirdeye) > 1.0, "an instant is safer");
    let Some(palm) = gem::gem("palm") else {
        return;
    };
    assert!(safety_factor(palm) < 1.0, "unsafe on block");
    assert!(safety_factor(spark) == 1.0);
    // the same skill made unsafe scores less, made instant more
    let unsafe_palm = power_score(palm);
    let plain = Gem {
        id: "palmcopy",
        ..*palm
    };
    assert!(power_score(&plain) > unsafe_palm);
    let instant = Gem {
        tags: &[Tag::Spell, Tag::Far, Tag::Instant],
        ..*spark
    };
    assert!(power_score(&instant) > power_score(spark));
}

#[test]
fn a_damaging_skill_binds_and_a_curse_a_buff_a_move_a_guard_a_heal_or_a_stance_never_does() {
    let binds = |id: &str| gem::gem(id).is_some_and(Gem::binds);
    for id in ["bolt", "orb", "fireball", "jet", "palm"] {
        assert!(binds(id), "{id}");
    }
    // a heal, a ward, a blink, a curse, an aura
    for id in ["heal", "ward", "blink", "hex", "slaughteraura"] {
        assert!(!binds(id), "{id}");
    }
    // no role of `UNBOUND` binds, whatever it deals
    for g in GEMS {
        if gem::UNBOUND.iter().any(|&t| g.tags().has(t)) {
            assert!(!g.binds(), "{}", g.id);
        }
    }
}
