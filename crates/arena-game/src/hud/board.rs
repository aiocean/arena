//! The live matches' board: under the bars, the match's number in the server's run, the round, and the score of the
//! matches so far between the two fighters (by fighter, each name on the side its fighter stands this match); and when
//! a match is over, a card in the middle of the screen with its result (who won, by how many rounds, by a forfeit or
//! as a draw), the score with it counted, and how long until the next, for the rest the server takes between matches
//! (`arena_link::moq::BETWEEN_MATCHES_MS`). Only a page watching the live matches shows it.
//!
//! Text in the screen's own pixels (UI nodes), in Be Vietnam Pro Bold, so the names keep their marks.

use crate::layout::{BARS_TALL, Layout};
use crate::live::Live;
use crate::rules::Shown;
use arena_duel::data::Look;
use arena_duel::duel::TICK;
use arena_duel::view::DuelView;
use arena_link::moq::BETWEEN_MATCHES_MS;
use arena_link::{Ended, Tally};
use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::window::PrimaryWindow;

const FONT: &[u8] = include_bytes!("../../assets/fonts/BeVietnamPro-Bold-vi.ttf");

const GOLD: (u8, u8, u8) = (0xf0, 0xcf, 0x7a);
const PALE: (u8, u8, u8) = (0xe8, 0xe0, 0xf0);
const DIM: (u8, u8, u8) = (0x9a, 0x92, 0xb0);

/// A match's result as the card shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub title: String,
    pub how: String,
    pub score: String,
}

/// The fighter index a score counts by (0 Vương Lâm, 1 Thất Thải Đạo Nhân).
fn fighter(look: Look) -> usize {
    usize::from(look != Look::VuongLam)
}

/// The score line, each fighter's name on the side it stands in `looks` (corner 0's on the left).
pub fn score_line(tally: &Tally, looks: [Look; 2]) -> String {
    let [a, b] = looks;
    let mut s = format!(
        "{} {} – {} {}",
        a.display(),
        tally.wins[fighter(a)],
        tally.wins[fighter(b)],
        b.display()
    );
    if tally.draws > 0 {
        let d = tally.draws;
        s += &format!(" · {d} {}", if d == 1 { "draw" } else { "draws" });
    }
    s
}

/// The top line: the match's number and the round being fought.
pub fn match_line(tally: &Tally, round: u32) -> String {
    format!("Match {} · Round {}", tally.number, round.max(1))
}

/// The card for a match that ended as `ended`, its fight as it stands at the end (`view`), `forfeited` the corners that
/// gave up, and `tally` the score before it.
pub fn card(ended: Ended, view: &DuelView, forfeited: [bool; 2], tally: &Tally) -> Card {
    let looks = [view.fighters[0].look, view.fighters[1].look];
    let [w0, w1] = view.wins;
    match ended {
        Ended::Winner(c) => {
            let c = usize::from(c.min(1));
            let (won, lost) = (view.wins[c], view.wins[1 - c]);
            let rounds = format!("{won} round{} to {lost}", if won == 1 { "" } else { "s" });
            Card {
                title: format!("{} wins", looks[c].display()),
                how: if forfeited[1 - c] {
                    format!("{} gave up · {rounds}", looks[1 - c].display())
                } else {
                    rounds
                },
                score: score_line(&tally.after(Some(fighter(looks[c]))), looks),
            }
        }
        Ended::Draw => Card {
            title: "Draw".into(),
            how: format!(
                "Two minutes ran out · {w0} round{} to {w1}",
                if w0 == 1 { "" } else { "s" }
            ),
            score: score_line(&tally.after(None), looks),
        },
    }
}

#[derive(Component)]
struct Strip;
#[derive(Component)]
struct StripText(usize);
#[derive(Component)]
struct CardPanel;
#[derive(Component)]
struct CardText(usize);

pub struct BoardPlugin;

impl Plugin for BoardPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn)
            .add_systems(Update, show.in_set(crate::Frame::Upload));
    }
}

fn spawn(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    let font = fonts.add(Font::from_bytes(FONT.to_vec()));
    let text = |i: usize| {
        (
            Text::new(""),
            TextFont::from(font.clone()),
            TextShadow::default(),
            i,
        )
    };
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(50),
            Strip,
        ))
        .with_children(|p| {
            for i in 0..2 {
                let (t, f, s, i) = text(i);
                p.spawn((
                    t,
                    f,
                    s,
                    TextColor(rgb(if i == 0 { GOLD } else { PALE })),
                    StripText(i),
                ));
            }
        });
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                display: Display::None,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(60),
            CardPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(28.0), Val::Px(18.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    row_gap: Val::Px(6.0),
                    ..default()
                },
                BackgroundColor(Color::srgba_u8(18, 13, 24, 225)),
                BorderColor::all(Color::srgb_u8(0xb8, 0x89, 0x3c)),
            ))
            .with_children(|p| {
                for (i, c) in [GOLD, PALE, PALE, DIM].into_iter().enumerate() {
                    let (t, f, s, i) = text(i);
                    p.spawn((t, f, s, TextColor(rgb(c)), CardText(i)));
                }
            });
        });
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::srgb_u8(r, g, b)
}

/// The card on screen: the fight it is for, what it says, and when it came up (seconds of the app's clock).
struct Up {
    fight: u32,
    card: Card,
    since: f32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn show(
    live: NonSend<Live>,
    shown: Res<Shown>,
    layout: Res<Layout>,
    time: Res<Time>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut strip: Query<&mut Node, (With<Strip>, Without<CardPanel>)>,
    mut panel: Query<&mut Node, (With<CardPanel>, Without<Strip>)>,
    mut strip_text: Query<(&StripText, &mut Text, &mut TextFont), Without<CardText>>,
    mut card_text: Query<(&CardText, &mut Text, &mut TextFont), Without<StripText>>,
    mut up: Local<Option<Up>>,
) {
    let (Ok(mut strip), Ok(mut panel)) = (strip.single_mut(), panel.single_mut()) else {
        return;
    };
    let view = shown.view.as_ref().filter(|_| live.on);
    let m = live.watching.current();
    // the card comes up once the page has played the ended match to its last tick, and stays until the next fight
    if up.as_ref().is_some_and(|u| u.fight != shown.fight) {
        *up = None;
    }
    if let (Some(view), Some(m)) = (view, m)
        && let Some(ended) = m.ended
        && up.is_none()
        && (view.time / TICK).round() as u32 >= m.through
    {
        let forfeited = [0u8, 1].map(|c| m.logs[usize::from(c)].borrow().forfeit.is_some());
        *up = Some(Up {
            fight: shown.fight,
            card: card(ended, view, forfeited, &m.tally),
            since: time.elapsed_secs(),
        });
    }
    // a world pixel in logical pixels, as the tip measures it
    let px = (layout.scale / layout.dpr).max(2.0) / 4.0;
    let (Some(view), Some(window)) = (view, windows.single().ok()) else {
        strip.display = Display::None;
        panel.display = Display::None;
        return;
    };

    // the strip just under the medallions, below the plaque a narrow arena hangs under the bars
    let (_, oy) = ((layout.w % 2.0) / 2.0, (layout.h % 2.0) / 2.0);
    let world_y = layout.bar_top + BARS_TALL + 3.0;
    let logical = ((world_y - layout.h / 2.0 + oy) * layout.scale) / window.scale_factor()
        + window.height() / 2.0;
    strip.display = Display::Flex;
    strip.top = Val::Px(logical);
    let looks = [view.fighters[0].look, view.fighters[1].look];
    let tally = m.map(|m| m.tally).unwrap_or_default();
    let lines = [match_line(&tally, view.round), score_line(&tally, looks)];
    for (StripText(i), mut text, mut font) in &mut strip_text {
        set(&mut text, &lines[*i]);
        font.font_size = FontSize::Px(if *i == 0 { 15.0 } else { 13.0 } * px);
    }

    let Some(u) = up.as_ref() else {
        panel.display = Display::None;
        return;
    };
    panel.display = Display::Flex;
    let left = (BETWEEN_MATCHES_MS as f32 / 1000.0 - (time.elapsed_secs() - u.since)).ceil();
    let next = if left >= 1.0 {
        format!("Next match in {left} s")
    } else {
        "Next match soon".into()
    };
    let lines = [&u.card.title, &u.card.how, &u.card.score, &next];
    for (CardText(i), mut text, mut font) in &mut card_text {
        set(&mut text, lines[*i]);
        let size = [30.0, 15.0, 14.0, 12.0][*i];
        font.font_size = FontSize::Px(size * px);
    }
}

/// Set a text only when it changes (a change lays it out again).
fn set(text: &mut Mut<Text>, s: &str) {
    if text.0 != s {
        text.0 = s.to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_score_reads_by_fighter_on_the_side_each_stands() {
        let t = Tally {
            number: 9,
            wins: [5, 3],
            draws: 0,
        };
        assert_eq!(
            score_line(&t, [Look::VuongLam, Look::ThatThai]),
            "Vương Lâm 5 – 3 Thất Thải Đạo Nhân"
        );
        assert_eq!(
            score_line(&t, [Look::ThatThai, Look::VuongLam]),
            "Thất Thải Đạo Nhân 3 – 5 Vương Lâm"
        );
        let d = Tally { draws: 2, ..t };
        assert!(score_line(&d, [Look::VuongLam, Look::ThatThai]).ends_with("· 2 draws"));
        assert_eq!(match_line(&t, 2), "Match 9 · Round 2");
    }

    #[test]
    fn the_card_names_the_winner_the_rounds_and_counts_the_match() {
        use arena_duel::duel::{Duel, DuelConfig};
        use arena_duel::seat::{Log, MakeSeat, Replay, Seat};
        use std::cell::RefCell;
        use std::rc::Rc;
        let seat = || -> MakeSeat {
            let log = Rc::new(RefCell::new(Log::default()));
            Box::new(move |_s, _m| Box::new(Replay::live(log)) as Box<dyn Seat>)
        };
        let config = DuelConfig {
            seed: 4,
            left: 88.0,
            right: 1192.0,
            ground: 632.0,
            sky: 196.0,
            effects: arena_duel::effects::all(),
        };
        let mut v = arena_duel::view::view(Duel::new_drawn(config, [seat(), seat()]).state());
        // seed 4: Vương Lâm in corner 0
        assert_eq!(v.fighters[0].look, Look::VuongLam);
        v.wins = [1, 2];
        let t = Tally {
            number: 4,
            wins: [2, 1],
            draws: 0,
        };
        let c = card(Ended::Winner(1), &v, [false, false], &t);
        assert_eq!(c.title, "Thất Thải Đạo Nhân wins");
        assert_eq!(c.how, "2 rounds to 1");
        assert_eq!(c.score, "Vương Lâm 2 – 2 Thất Thải Đạo Nhân");
        let c = card(Ended::Winner(1), &v, [true, false], &t);
        assert_eq!(c.how, "Vương Lâm gave up · 2 rounds to 1");
        v.wins = [1, 1];
        let c = card(Ended::Draw, &v, [false, false], &t);
        assert_eq!(c.title, "Draw");
        assert!(c.score.ends_with("· 1 draw"));
    }
}
