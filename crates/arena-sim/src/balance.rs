//! The balance tool (design 12.2 and 12.3), run by `examples/balance.rs`: random legal loadout pairs
//! (`loadout::random`), each played both ways over a set of seeds, and for every skill, support and keystone its
//! picks, games, inclusion win rate minus 50, used rate, average damage dealt, and the average distance between the
//! fighters at its casts and its hits; then a C/B/A/S tier by inclusion win rate quantile (S the top 15%, A the next
//! 35%, B the next 35%, C the last 15%).
//!
//! Measured under the simulator's scripted brain, not the real fight: a first ranking, not the tiers' last word.

use std::fmt::Write as _;

use arena_calc::loadout::{self, Loadout};

use crate::fight::{MatchResult, play_match};
use crate::report::Place;
use crate::rng::Rng;

/// The tiers' quantile cuts from the top: S above `TIER_CUTS[0]`, A above `[1]`, B above `[2]`, C the rest.
pub const TIER_CUTS: [f64; 3] = [0.15, 0.50, 0.85];

/// What to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// Seeds each pair is played over (both ways on each).
    pub seeds: u32,
    /// Random loadout pairs.
    pub pairs: u32,
    /// The seed the pairs and the matches' seeds come from.
    pub seed: u64,
    /// Threads to play on (results do not depend on it).
    pub threads: usize,
}

/// What a row measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Gem,
    Support,
    Keystone,
}

/// One skill's, support's or keystone's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub kind: Kind,
    pub id: String,
    /// Loadouts that held it.
    pub picks: u32,
    /// Matches played by a loadout holding it, and their score (a win 1, a drawn match 0.5).
    pub games: u32,
    pub score: f64,
    /// Matches in which it was used (a support: its skill was).
    pub used: u32,
    /// Damage it dealt (a support: its skill), summed over its games.
    pub damage: f64,
    /// Sums of distance at its casts and hits, and their counts.
    pub dist_cast: f64,
    pub casts: u32,
    pub dist_hit: f64,
    pub hits: u32,
    /// Its tier, by inclusion win rate among its kind.
    pub tier: char,
}

impl Row {
    fn new(kind: Kind, id: String) -> Row {
        Row {
            kind,
            id,
            picks: 0,
            games: 0,
            score: 0.0,
            used: 0,
            damage: 0.0,
            dist_cast: 0.0,
            casts: 0,
            dist_hit: 0.0,
            hits: 0,
            tier: '-',
        }
    }

    /// Inclusion win rate minus 50, in points.
    pub fn wr(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            (self.score / f64::from(self.games) - 0.5) * 100.0
        }
    }

    /// The share of its games it was used in.
    pub fn used_rate(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            f64::from(self.used) / f64::from(self.games)
        }
    }

    /// Average damage a game.
    pub fn avg_damage(&self) -> f64 {
        if self.games == 0 {
            0.0
        } else {
            self.damage / f64::from(self.games)
        }
    }

    /// Average distance at a cast and at a hit (`NaN`-free: 0 with none).
    pub fn avg_cast(&self) -> f64 {
        if self.casts == 0 {
            0.0
        } else {
            self.dist_cast / f64::from(self.casts)
        }
    }

    pub fn avg_hit(&self) -> f64 {
        if self.hits == 0 {
            0.0
        } else {
            self.dist_hit / f64::from(self.hits)
        }
    }
}

/// The whole run's numbers.
#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub config: Config,
    pub matches: u32,
    /// Matches seat 0 won, drawn, and rounds cut by the time limit, of rounds played.
    pub seat0_wins: u32,
    pub drawn: u32,
    pub rounds: u32,
    pub cut: u32,
    /// Average round length in seconds.
    pub round_seconds: f64,
    pub rows: Vec<Row>,
}

fn row<'a>(rows: &'a mut Vec<Row>, kind: Kind, id: &str) -> &'a mut Row {
    let i = match rows.iter().position(|r| r.kind == kind && r.id == id) {
        Some(i) => i,
        None => {
            rows.push(Row::new(kind, id.to_string()));
            rows.len() - 1
        }
    };
    &mut rows[i]
}

/// A match's seed from the run's seed, the pair and the seed's index.
fn match_seed(seed: u64, pair: u32, s: u32) -> u64 {
    Rng::salted(seed, (u64::from(pair) << 32) | u64::from(s)).next_u64()
}

/// Draws `config.pairs` legal loadout pairs.
pub fn pairs(config: &Config) -> Vec<(Loadout, Loadout)> {
    let mut dice = Rng::salted(config.seed, 7);
    let mut out = Vec::new();
    let mut tries = 0;
    while out.len() < config.pairs as usize && tries < config.pairs * 10 + 10 {
        tries += 1;
        let a = loadout::random(&mut || dice.next_f64());
        let b = loadout::random(&mut || dice.next_f64());
        if let (Some(a), Some(b)) = (a, b) {
            out.push((a, b));
        }
    }
    out
}

/// Plays one pair over every seed, both ways: (match, which loadout held seat 0: 0 for `a`).
fn play_pair(config: &Config, i: u32, a: &Loadout, b: &Loadout) -> Vec<(MatchResult, usize)> {
    let mut out = Vec::new();
    for s in 0..config.seeds {
        let seed = match_seed(config.seed, i, s);
        out.push((play_match(seed, a, b), 0));
        out.push((play_match(seed, b, a), 1));
    }
    out
}

/// Plays the run and measures it.
pub fn run(config: Config) -> Table {
    let pairs = pairs(&config);
    let threads = config.threads.max(1).min(pairs.len().max(1));
    let mut played: Vec<Vec<(MatchResult, usize)>> = vec![Vec::new(); pairs.len()];
    std::thread::scope(|scope| {
        let chunk = pairs.len().div_ceil(threads).max(1);
        let handles: Vec<_> = pairs
            .chunks(chunk)
            .enumerate()
            .map(|(c, ps)| {
                let cfg = config;
                scope.spawn(move || {
                    ps.iter()
                        .enumerate()
                        .map(|(j, (a, b))| play_pair(&cfg, (c * chunk + j) as u32, a, b))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for (c, h) in handles.into_iter().enumerate() {
            if let Ok(rs) = h.join() {
                for (j, r) in rs.into_iter().enumerate() {
                    if let Some(slot) = played.get_mut(c * chunk + j) {
                        *slot = r;
                    }
                }
            }
        }
    });

    let mut t = Table {
        config,
        matches: 0,
        seat0_wins: 0,
        drawn: 0,
        rounds: 0,
        cut: 0,
        round_seconds: 0.0,
        rows: Vec::new(),
    };
    let mut seconds = 0.0;
    for ((a, b), ms) in pairs.iter().zip(&played) {
        for (m, first) in ms {
            t.matches += 1;
            match m.winner {
                Some(0) => t.seat0_wins += 1,
                None => t.drawn += 1,
                _ => {}
            }
            for r in &m.rounds {
                t.rounds += 1;
                t.cut += u32::from(r.cut);
                seconds += r.seconds;
            }
            for seat in 0..2 {
                let l = if (seat == 0) == (*first == 0) { a } else { b };
                let score = match m.winner {
                    Some(w) if w == seat => 1.0,
                    Some(_) => 0.0,
                    None => 0.5,
                };
                tally(&mut t.rows, l, &m.sides[seat], score);
            }
        }
    }
    // picks: each loadout once
    for (a, b) in &pairs {
        for l in [a, b] {
            for s in &l.actives {
                row(&mut t.rows, Kind::Gem, &s.skill).picks += 1;
                for x in &s.supports {
                    row(&mut t.rows, Kind::Support, x).picks += 1;
                }
            }
            row(&mut t.rows, Kind::Gem, &l.ultimate).picks += 1;
            for k in &l.keystones {
                row(&mut t.rows, Kind::Keystone, &format!("{k:?}")).picks += 1;
            }
        }
    }
    t.round_seconds = if t.rounds == 0 {
        0.0
    } else {
        seconds / f64::from(t.rounds)
    };
    for kind in [Kind::Gem, Kind::Support, Kind::Keystone] {
        let mut idx: Vec<usize> = (0..t.rows.len())
            .filter(|&i| t.rows[i].kind == kind && t.rows[i].games > 0)
            .collect();
        idx.sort_by(|&x, &y| {
            t.rows[y]
                .wr()
                .total_cmp(&t.rows[x].wr())
                .then_with(|| t.rows[x].id.cmp(&t.rows[y].id))
        });
        let n = idx.len() as f64;
        for (rank, &i) in idx.iter().enumerate() {
            let q = (rank as f64 + 0.5) / n;
            t.rows[i].tier = if q <= TIER_CUTS[0] {
                'S'
            } else if q <= TIER_CUTS[1] {
                'A'
            } else if q <= TIER_CUTS[2] {
                'B'
            } else {
                'C'
            };
        }
    }
    t.rows.sort_by(|x, y| {
        (x.kind as u8)
            .cmp(&(y.kind as u8))
            .then(y.wr().total_cmp(&x.wr()))
            .then_with(|| x.id.cmp(&y.id))
    });
    t
}

/// Adds one side of one match to the rows of what its loadout holds.
fn tally(rows: &mut Vec<Row>, l: &Loadout, side: &crate::report::SideReport, score: f64) {
    for line in &side.skills {
        let id = match line.place {
            Place::Basic => continue,
            Place::Active(_) | Place::Ultimate => line.id,
        };
        let r = row(rows, Kind::Gem, id);
        r.games += 1;
        r.score += score;
        r.used += u32::from(line.used());
        r.damage += line.damage;
        r.dist_cast += line.dist_cast;
        r.casts += line.casts;
        r.dist_hit += line.dist_hit;
        r.hits += line.landed;
        if let Place::Active(i) = line.place
            && let Some(slot) = l.actives.get(i)
        {
            for x in &slot.supports {
                let r = row(rows, Kind::Support, x);
                r.games += 1;
                r.score += score;
                r.used += u32::from(line.used());
                r.damage += line.damage;
                r.dist_cast += line.dist_cast;
                r.casts += line.casts;
                r.dist_hit += line.dist_hit;
                r.hits += line.landed;
            }
        }
    }
    for k in &l.keystones {
        let r = row(rows, Kind::Keystone, &format!("{k:?}"));
        r.games += 1;
        r.score += score;
        r.used += 1;
        r.damage += side.damage;
    }
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Gem => "gem",
        Kind::Support => "support",
        Kind::Keystone => "keystone",
    }
}

impl Table {
    /// The table as text: the run's totals, then each kind's rows best first.
    pub fn text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(
            s,
            "{} matches ({} pairs x {} seeds x 2), {} rounds, avg {:.1} s, {:.1}% cut; seat 0 won {:.1}%, {} drawn",
            self.matches,
            self.config.pairs,
            self.config.seeds,
            self.rounds,
            self.round_seconds,
            100.0 * f64::from(self.cut) / f64::from(self.rounds.max(1)),
            100.0 * f64::from(self.seat0_wins) / f64::from(self.matches.max(1)),
            self.drawn
        );
        for kind in [Kind::Gem, Kind::Support, Kind::Keystone] {
            let _ = writeln!(
                s,
                "\n{:<22} tier  picks  games   WR-50  used%   avg dmg  d@cast  d@hit",
                kind_name(kind)
            );
            for r in self.rows.iter().filter(|r| r.kind == kind) {
                let _ = writeln!(
                    s,
                    "{:<22} {:>4} {:>6} {:>6} {:>+7.1} {:>6.0} {:>9.0} {:>7.2} {:>6.2}",
                    r.id,
                    r.tier,
                    r.picks,
                    r.games,
                    r.wr(),
                    r.used_rate() * 100.0,
                    r.avg_damage(),
                    r.avg_cast(),
                    r.avg_hit()
                );
            }
        }
        s
    }

    /// The table as JSON.
    pub fn json(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{{\"seeds\":{},\"pairs\":{},\"seed\":{},\"matches\":{},\"seat0Wins\":{},\"drawn\":{},\"rounds\":{},\"cut\":{},\"roundSeconds\":{:.3},\"rows\":[",
            self.config.seeds,
            self.config.pairs,
            self.config.seed,
            self.matches,
            self.seat0_wins,
            self.drawn,
            self.rounds,
            self.cut,
            self.round_seconds
        );
        for (i, r) in self.rows.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            let id: String = r
                .id
                .chars()
                .filter(|c| !matches!(c, '"' | '\\') && !c.is_control())
                .collect();
            let _ = write!(
                s,
                "{{\"kind\":\"{}\",\"id\":\"{}\",\"tier\":\"{}\",\"picks\":{},\"games\":{},\"wrMinus50\":{:.3},\"usedRate\":{:.4},\"avgDamage\":{:.1},\"avgDistCast\":{:.3},\"avgDistHit\":{:.3}}}",
                kind_name(r.kind),
                id,
                r.tier,
                r.picks,
                r.games,
                r.wr(),
                r.used_rate(),
                r.avg_damage(),
                r.avg_cast(),
                r.avg_hit()
            );
        }
        s.push_str("]}");
        s
    }
}
