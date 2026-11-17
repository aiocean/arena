//! What a host measures of a bot, whichever runs it (`host`: wasmtime, `onnx`: tract): each decision's wall time
//! (and fuel, for a module) as metric events, each fault as an `arena::bot` event, and an instance's totals, with
//! its think-time and fuel distributions, as `bot.end` when it is dropped (`ops/observe/README.md`).
//!
//! A distribution is a fixed log-linear histogram (`Dist`): 32 buckets to each power of two, made once with the
//! instance, so a decision costs a clock read and an increment, never an allocation, and the same samples always give
//! the same percentiles. A percentile is the middle of its bucket, within 1/64 of the true sample (exact below 32).

use arena_duel::seat::Fault;
use std::sync::Arc;
use std::time::Duration;
use tracing::{Level, event};

/// Sub-buckets to each power of two, as bits: 32, so a bucket is at most 1/32 of its value wide.
const SUB_BITS: u32 = 5;
const SUB: u64 = 1 << SUB_BITS;
/// Buckets enough for any `u64`.
const BUCKETS: usize = ((64 - SUB_BITS as usize) + 1) * SUB as usize;

/// A distribution of `u64` samples: their count, sum, least and most, and a log-linear histogram.
#[derive(Clone)]
pub struct Dist {
    counts: Box<[u32; BUCKETS]>,
    n: u64,
    sum: u128,
    min: u64,
    max: u64,
}

impl Default for Dist {
    fn default() -> Dist {
        Dist {
            counts: Box::new([0; BUCKETS]),
            n: 0,
            sum: 0,
            min: u64::MAX,
            max: 0,
        }
    }
}

fn index(v: u64) -> usize {
    if v < SUB {
        return v as usize;
    }
    let shift = 63 - v.leading_zeros() - SUB_BITS;
    ((u64::from(shift) + 1) * SUB + ((v >> shift) - SUB)) as usize
}

/// The least and most value of bucket `i`.
fn bounds(i: usize) -> (u64, u64) {
    let i = i as u64;
    if i < SUB {
        return (i, i);
    }
    let shift = i / SUB - 1;
    let lo = (i % SUB + SUB) << shift;
    (lo, lo + ((1_u64 << shift) - 1))
}

impl Dist {
    pub fn add(&mut self, v: u64) {
        let c = &mut self.counts[index(v)];
        *c = c.saturating_add(1);
        self.n += 1;
        self.sum += u128::from(v);
        self.min = self.min.min(v);
        self.max = self.max.max(v);
    }

    pub fn count(&self) -> u64 {
        self.n
    }

    pub fn mean(&self) -> Option<f64> {
        (self.n > 0).then(|| self.sum as f64 / self.n as f64)
    }

    pub fn max(&self) -> Option<u64> {
        (self.n > 0).then_some(self.max)
    }

    /// The `q` quantile (0 to 1) by nearest rank: the middle of the bucket the rank falls in, kept within the least
    /// and most sample (the first and last rank are those exactly). None with no samples.
    pub fn quantile(&self, q: f64) -> Option<u64> {
        if self.n == 0 {
            return None;
        }
        // the rank in whole numbers: q to a millionth, then rounded up
        let millionths = (q.clamp(0.0, 1.0) * 1e6) as u64;
        let rank = (u128::from(millionths) * u128::from(self.n))
            .div_ceil(1_000_000)
            .max(1) as u64;
        // the least and most samples are known exactly
        if rank == 1 {
            return Some(self.min);
        }
        if rank >= self.n {
            return Some(self.max);
        }
        let mut seen = 0_u64;
        for (i, &c) in self.counts.iter().enumerate() {
            seen += u64::from(c);
            if seen >= rank {
                let (lo, hi) = bounds(i);
                return Some((lo + (hi - lo) / 2).clamp(self.min, self.max));
            }
        }
        Some(self.max)
    }
}

/// An instance's totals as `bot.end` reports them; the fuel ones are None for a runtime without fuel (ONNX).
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub decisions: u64,
    pub matches: u64,
    pub faults: u64,
    pub think_mean_ms: Option<f64>,
    pub think_p50_ms: Option<f64>,
    pub think_p95_ms: Option<f64>,
    pub think_p99_ms: Option<f64>,
    pub think_max_ms: Option<f64>,
    pub fuel_mean: Option<f64>,
    pub fuel_p50: Option<u64>,
    pub fuel_p95: Option<u64>,
    pub fuel_max: Option<u64>,
    pub fuel_cap: Option<i64>,
}

/// One bot instance's measures: what runs it (`runtime`, "wasm" or "onnx"), which bot, and what it did.
pub struct Tally {
    runtime: &'static str,
    bot: Arc<str>,
    decisions: u64,
    matches: u64,
    faults: u64,
    /// Think time, nanoseconds.
    think: Dist,
    /// Fuel a decision, and the cap on it, for a metered module.
    fuel: Option<(Dist, i64)>,
}

impl Tally {
    /// A tally for one instance; `fuel_cap` is the fuel a decision is given, None for a runtime without fuel.
    pub fn new(runtime: &'static str, bot: Arc<str>, fuel_cap: Option<i64>) -> Tally {
        Tally {
            runtime,
            bot,
            decisions: 0,
            matches: 0,
            faults: 0,
            think: Dist::default(),
            fuel: fuel_cap.map(|cap| (Dist::default(), cap)),
        }
    }

    /// A fight was started on the instance.
    pub fn started(&mut self) {
        self.matches += 1;
    }

    /// A decision took `took` and burned `fuel` (for a module).
    pub fn decided(&mut self, took: Duration, fuel: Option<i64>) {
        self.decisions += 1;
        self.think
            .add(u64::try_from(took.as_nanos()).unwrap_or(u64::MAX));
        let (runtime, bot) = (self.runtime, &*self.bot);
        match (fuel, &mut self.fuel) {
            (Some(used), Some((dist, _))) => {
                dist.add(u64::try_from(used).unwrap_or(0));
                event!(target: "arena::metrics", Level::INFO, {
                    histogram.arena_bot_think_seconds = took.as_secs_f64(), histogram.arena_bot_fuel = used as f64,
                    runtime = runtime, bot = bot,
                });
            }
            _ => event!(target: "arena::metrics", Level::INFO, {
                histogram.arena_bot_think_seconds = took.as_secs_f64(), runtime = runtime, bot = bot,
            }),
        }
    }

    /// A call failed: `kind` is trap or out_of_fuel for a module, error for ONNX.
    pub fn fault(&mut self, call: &'static str, kind: &'static str, f: &Fault) {
        self.faults += 1;
        let (runtime, bot, n) = (self.runtime, &*self.bot, self.faults);
        // a bot that faults on every call is told about ten times, then every thousandth (all are counted)
        if n <= 10 || n.is_multiple_of(1000) {
            event!(target: "arena::bot", Level::WARN, {
                runtime = runtime, bot = bot, call = call, kind = kind, fault = %f.0, faults = n,
            }, "bot.fault");
        }
        event!(target: "arena::metrics", Level::INFO, {
            monotonic_counter.arena_bot_faults_total = 1_u64, call = call, kind = kind, runtime = runtime, bot = bot,
        });
    }

    pub fn summary(&self) -> Summary {
        let ms = |ns: Option<u64>| ns.map(|ns| ns as f64 / 1e6);
        let fuel = self.fuel.as_ref();
        Summary {
            decisions: self.decisions,
            matches: self.matches,
            faults: self.faults,
            think_mean_ms: self.think.mean().map(|ns| ns / 1e6),
            think_p50_ms: ms(self.think.quantile(0.5)),
            think_p95_ms: ms(self.think.quantile(0.95)),
            think_p99_ms: ms(self.think.quantile(0.99)),
            think_max_ms: ms(self.think.max()),
            fuel_mean: fuel.and_then(|(d, _)| d.mean()),
            fuel_p50: fuel.and_then(|(d, _)| d.quantile(0.5)),
            fuel_p95: fuel.and_then(|(d, _)| d.quantile(0.95)),
            fuel_max: fuel.and_then(|(d, _)| d.max()),
            fuel_cap: fuel.map(|(_, cap)| *cap),
        }
    }
}

impl Drop for Tally {
    fn drop(&mut self) {
        if self.decisions == 0 && self.faults == 0 {
            return;
        }
        let s = self.summary();
        event!(target: "arena::bot", Level::INFO, {
            runtime = self.runtime, bot = &*self.bot, decisions = s.decisions, matches = s.matches,
            faults = s.faults, think_mean_ms = s.think_mean_ms, think_p50_ms = s.think_p50_ms,
            think_p95_ms = s.think_p95_ms, think_p99_ms = s.think_p99_ms, think_max_ms = s.think_max_ms,
            fuel_mean = s.fuel_mean, fuel_p50 = s.fuel_p50, fuel_p95 = s.fuel_p95, fuel_max = s.fuel_max,
            fuel_most = s.fuel_max, fuel_cap = s.fuel_cap,
        }, "bot.end");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_cover_every_value_in_order() {
        for v in (0..5000).chain([u64::MAX / 3, u64::MAX - 1, u64::MAX]) {
            let (lo, hi) = bounds(index(v));
            assert!(lo <= v && v <= hi, "{v} in [{lo}, {hi}]");
        }
        assert_eq!(index(u64::MAX), BUCKETS - 1);
        // a bucket is at most 1/32 of its value wide
        for i in SUB as usize..BUCKETS {
            let (lo, hi) = bounds(i);
            assert!((hi - lo) <= lo / SUB, "bucket {i}");
            assert_eq!(index(lo), i);
            assert_eq!(index(hi), i);
        }
    }

    #[test]
    fn small_samples_are_exact() {
        let mut d = Dist::default();
        assert_eq!(d.quantile(0.5), None);
        for v in 1..=20 {
            d.add(v);
        }
        assert_eq!(d.quantile(0.5), Some(10));
        assert_eq!(d.quantile(0.95), Some(19));
        assert_eq!(d.quantile(1.0), Some(20));
        assert_eq!(d.quantile(0.0), Some(1));
        assert_eq!(d.max(), Some(20));
        assert_eq!(d.mean(), Some(10.5));
    }

    #[test]
    fn percentiles_of_known_samples_within_a_bucket() {
        // 1..=1000 ms in nanoseconds, shuffled by a stride: p50 = 500 ms, p95 = 950, p99 = 990, max 1000
        let mut d = Dist::default();
        for k in 0..1000_u64 {
            d.add(((k * 389) % 1000 + 1) * 1_000_000);
        }
        let near = |got: Option<u64>, want: f64| {
            let got = got.unwrap_or(0) as f64;
            assert!((got - want).abs() <= want / 64.0, "{got} vs {want}");
        };
        near(d.quantile(0.5), 500e6);
        near(d.quantile(0.95), 950e6);
        near(d.quantile(0.99), 990e6);
        assert_eq!(d.max(), Some(1_000_000_000));
        assert_eq!(d.quantile(1.0), Some(1_000_000_000));
        assert_eq!(d.mean(), Some(500.5e6));
    }

    #[test]
    fn slow_decisions_show_in_p99_and_max_not_p50() {
        let mut t = Tally::new("onnx", "house".into(), None);
        for _ in 0..980 {
            t.decided(Duration::from_micros(200), None);
        }
        for _ in 0..20 {
            t.decided(Duration::from_millis(30), None);
        }
        let s = t.summary();
        assert_eq!(s.decisions, 1000);
        let p50 = s.think_p50_ms.unwrap_or(0.0);
        assert!((p50 - 0.2).abs() < 0.2 / 64.0, "{p50}");
        assert!(s.think_p95_ms.unwrap_or(0.0) < 0.21);
        let p99 = s.think_p99_ms.unwrap_or(0.0);
        assert!(
            (p99 - 30.0).abs() < 30.0 / 64.0,
            "the 990th of 1000 is a slow one: {p99}"
        );
        assert_eq!(s.think_max_ms, Some(30.0));
        assert_eq!((s.fuel_p95, s.fuel_cap), (None, None));
    }
}
