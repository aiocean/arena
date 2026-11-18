//! The one-cast cap as a rule of the data (rules, "One cast, 40% of a life"; `arena_calc::cap`): every skill of the
//! pool, bare, deals at most `CAST_CAP` of a life and its shield in one cast, at hit plus the damage over time it
//! lays. Supported forms are not held to it yet (they are measured by `arena-sim`'s `cap` example and tuned by the
//! simulator's win rates).

use arena_calc::cap::{CAST_CAP, cap_life, cast_of, damaging, fitting, skill_of};
use arena_calc::event::{self, Scale};
use arena_calc::gem::{self, BASE_LEVEL, GEMS};
use arena_calc::support::SUPPORTS;

#[test]
fn no_bare_cast_deals_more_than_the_cap() {
    let of = cap_life();
    let lines: Vec<String> = GEMS
        .iter()
        .filter(|g| damaging(g))
        .filter_map(|g| {
            let d = cast_of(g, &[]);
            d.over(of).then(|| {
                format!(
                    "{}: {:.1}% ({:.0} at hit, {:.0} over time)",
                    g.id,
                    100.0 * d.share(of),
                    d.hit,
                    d.over_time
                )
            })
        })
        .collect();
    assert!(
        lines.is_empty(),
        "{} skills over {:.0}% of {of}:\n{}",
        lines.len(),
        100.0 * CAST_CAP,
        lines.join("\n")
    );
}

/// The cap reads what a cast lays, not only its blow: a bleeding blow counts its bleed, a curse its strikes.
#[test]
fn the_cap_counts_damage_over_time() {
    let slash = gem::gem("slash").expect("slash");
    let d = cast_of(slash, &[]);
    assert!(d.hit > 0.0 && d.over_time > 0.0, "{d:?}");
    let nine = gem::gem("ninedeaths").expect("ninedeaths");
    assert!(cast_of(nine, &[]).over_time > 0.0);
    // a support that adds damage shows in the measure (the supported forms are measured, not held)
    let bolt = gem::gem("bolt").expect("bolt");
    let more = fitting(&skill_of(bolt))
        .into_iter()
        .map(|s| cast_of(bolt, &[s]).total())
        .fold(0.0, f64::max);
    assert!(more > cast_of(bolt, &[]).total());
}

/// A triggered cast copies the skill it is hung on at no more than its level and its damage; one whose damage is
/// the event's (Liên Hoàn Suy Yếu) is bounded by that event.
#[test]
fn a_triggered_cast_is_no_stronger_than_the_cast() {
    for s in SUPPORTS {
        let Some(t) = s.trigger else {
            continue;
        };
        if let event::Effect::Cast(c) = t.does {
            if matches!(c.scale, Scale::Skill) {
                assert!(c.factor <= 1.0, "{}: factor {}", s.id, c.factor);
            }
            assert!(c.level <= BASE_LEVEL, "{}: level {}", s.id, c.level);
        }
    }
}
