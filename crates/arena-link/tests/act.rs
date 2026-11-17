//! Every `Choice` variant survives the trip to `Act` and back, and every `Frame` survives a postcard round-trip. A new
//! `Choice` variant that `Act` does not mirror fails to compile here (the match is exhaustive); one mirrored wrong
//! fails the equality.

use arena_link::{Act, Arena, Decision, Ended, Frame, Forfeit, Snapshot, Through, decode, encode};

const ARENA: Arena = Arena {
    left: 88.0,
    right: 1192.0,
    ground: 632.0,
    sky: 196.0,
};
use arena_mind::Choice;

fn every_choice() -> Vec<Choice> {
    vec![
        Choice::Move { dir: -1.0 },
        Choice::Jump,
        Choice::Duck { hold: 0.25 },
        Choice::Guard { hold: 0.5 },
        Choice::Fly { height: 48.0 },
        Choice::Meditate,
        Choice::Wait,
        Choice::Cast {
            skill: "thunder-bolt".into(),
            target: Some(120.0),
        },
        Choice::Cast {
            skill: "guard".into(),
            target: None,
        },
    ]
}

#[test]
fn choice_round_trips_through_act() {
    for c in every_choice() {
        let act = Act::from(&c);
        let back: Choice = act.into();
        assert_eq!(c, back, "a Choice variant did not survive Act");
    }
}

#[test]
fn act_round_trips_through_postcard() -> Result<(), postcard::Error> {
    for c in every_choice() {
        let act = Act::from(&c);
        let bytes = postcard::to_stdvec(&act)?;
        let back: Act = postcard::from_bytes(&bytes)?;
        assert_eq!(act, back);
    }
    Ok(())
}

#[test]
fn frames_round_trip_through_postcard() -> Result<(), postcard::Error> {
    let decisions: Vec<Decision> = every_choice()
        .iter()
        .enumerate()
        .map(|(i, c)| Decision {
            tick: i as u32,
            corner: (i % 2) as u8,
            act: Act::from(c),
        })
        .collect();
    let snapshot = Snapshot {
        rules: 1,
        seed: 42,
        arena: ARENA,
        corners: ["thunder".into(), "frost".into()],
        decisions: decisions.clone(),
        forfeits: vec![Forfeit { tick: 90, corner: 1 }],
        ended: Some(Ended::Winner(0)),
        ticks: 600,
    };
    let frames = vec![
        Frame::Keyframe(snapshot),
        Frame::Decision(decisions[0].clone()),
        Frame::Forfeit(Forfeit { tick: 5, corner: 0 }),
        Frame::Through(Through { tick: 61, sent_ms: 1_794_931_227_207 }),
        Frame::End(Ended::Draw),
    ];
    for f in frames {
        let bytes = encode(&f);
        assert!(!bytes.is_empty(), "a frame encoded to nothing");
        let back = decode(&bytes)?;
        assert_eq!(f, back);
    }
    Ok(())
}

#[test]
fn follow_builds_the_match_from_its_frames() -> Result<(), String> {
    use arena_link::{Applied, Follow, RULES_VERSION};
    let d = |tick, corner| Decision {
        tick,
        corner,
        act: Act::Jump,
    };
    let mut f = Follow::new();
    // a decision before any keyframe is not taken
    assert_eq!(f.apply(Frame::Decision(d(0, 0))), Applied::Ignored);
    let mut key = Snapshot::new(RULES_VERSION, 9, ARENA, ["a".into(), "b".into()]);
    key.decisions.push(d(0, 0));
    assert_eq!(f.apply(Frame::Keyframe(key)), Applied::Taken);
    // the keyframe's own decision repeated is not taken twice; a new one is
    assert_eq!(f.apply(Frame::Decision(d(0, 0))), Applied::Ignored);
    assert_eq!(f.apply(Frame::Decision(d(3, 1))), Applied::Taken);
    assert_eq!(f.through(), 0);
    assert_eq!(f.apply(Frame::Through(Through { tick: 12, sent_ms: 0 })), Applied::Taken);
    assert_eq!(f.apply(Frame::Through(Through { tick: 5, sent_ms: 0 })), Applied::Ignored);
    assert_eq!(f.through(), 12);
    assert_eq!(f.apply(Frame::End(Ended::Winner(1))), Applied::Taken);
    let s = f.snapshot().ok_or("no match held")?;
    assert_eq!(s.decisions.len(), 2);
    assert_eq!(s.ended, Some(Ended::Winner(1)));
    // other rules: the page reloads
    let other = Snapshot::new(RULES_VERSION + 1, 9, ARENA, ["a".into(), "b".into()]);
    assert_eq!(f.apply(Frame::Keyframe(other)), Applied::OtherRules(RULES_VERSION + 1));
    Ok(())
}
