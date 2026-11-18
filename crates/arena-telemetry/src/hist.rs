//! A distribution kept in log buckets: what a histogram metric line carries (`buckets`, `zero`) and what the query
//! tool's `--stats` merges across lines. Eight buckets to each doubling, so a percentile read from it is within about
//! 4.5% of the true sample (the bucket's geometric middle), held inside the exact least and most. Names, units and
//! bounds play no part: any two histograms merge.

use std::collections::BTreeMap;

/// Buckets to each doubling.
pub const PER_DOUBLING: f64 = 8.0;

/// The bucket of a positive value: bucket `i` holds `(2^(i/8), 2^((i+1)/8)]`. Zero, negatives and NaN have none.
pub fn bucket(v: f64) -> Option<i32> {
    (v > 0.0 && v.is_finite()).then(|| libm::ceil(libm::log2(v) * PER_DOUBLING) as i32 - 1)
}

/// The value a bucket stands for: its geometric middle.
pub fn middle(i: i32) -> f64 {
    libm::exp2((f64::from(i) + 0.5) / PER_DOUBLING)
}

/// A distribution: its count, sum, least and most exact, its shape in buckets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Hist {
    pub count: u64,
    pub sum: f64,
    pub min: f64,
    pub max: f64,
    /// Values of zero or less (a tick on time is 0 s late).
    pub zero: u64,
    pub buckets: BTreeMap<i32, u64>,
}

impl Hist {
    pub fn add(&mut self, v: f64) {
        if !v.is_finite() {
            return;
        }
        self.add_n(v, 1);
    }

    fn add_n(&mut self, v: f64, n: u64) {
        if self.count == 0 {
            (self.min, self.max) = (v, v);
        } else {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
        self.count += n;
        self.sum += v * n as f64;
        match bucket(v) {
            Some(i) => *self.buckets.entry(i).or_insert(0) += n,
            None => self.zero += n,
        }
    }

    /// Merge another distribution in.
    pub fn merge(&mut self, o: &Hist) {
        if o.count == 0 {
            return;
        }
        if self.count == 0 {
            (self.min, self.max) = (o.min, o.max);
        } else {
            self.min = self.min.min(o.min);
            self.max = self.max.max(o.max);
        }
        self.count += o.count;
        self.sum += o.sum;
        self.zero += o.zero;
        for (i, n) in &o.buckets {
            *self.buckets.entry(*i).or_insert(0) += n;
        }
    }

    pub fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }

    /// The `q` quantile (0 to 1), nearest rank, read from the buckets and held inside `[min, max]`.
    pub fn quantile(&self, q: f64) -> Option<f64> {
        if self.count == 0 {
            return None;
        }
        let rank = (libm::ceil(q.clamp(0.0, 1.0) * self.count as f64) as u64).max(1);
        if rank >= self.count {
            return Some(self.max);
        }
        let mut seen = self.zero;
        if rank <= seen {
            return Some(0.0_f64.clamp(self.min, self.max));
        }
        for (i, n) in &self.buckets {
            seen += n;
            if rank <= seen {
                return Some(middle(*i).clamp(self.min, self.max));
            }
        }
        Some(self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_holds_its_value_and_its_middle_is_close() {
        for v in [1e-6, 3.3e-4, 0.0167, 1.0, 2.0, 7.0, 1e7] {
            let i = bucket(v).unwrap_or(i32::MIN);
            let (lo, hi) = (
                libm::exp2(f64::from(i) / PER_DOUBLING),
                libm::exp2(f64::from(i + 1) / PER_DOUBLING),
            );
            assert!(
                lo < v * (1.0 + 1e-12) && v <= hi * (1.0 + 1e-12),
                "{v}: {lo}..{hi}"
            );
            assert!((middle(i) / v - 1.0).abs() < 0.05, "{v}");
        }
        assert_eq!(bucket(0.0), None);
        assert_eq!(bucket(-1.0), None);
    }

    #[test]
    fn quantiles_are_within_five_percent_and_merge() {
        let mut a = Hist::default();
        let mut b = Hist::default();
        for i in 1..=1000 {
            let v = f64::from(i) * 1e-4;
            if i % 2 == 0 { a.add(v) } else { b.add(v) }
        }
        a.merge(&b);
        assert_eq!(a.count, 1000);
        assert_eq!(a.max, 0.1);
        assert_eq!(a.min, 1e-4);
        for (q, want) in [(0.5, 0.05), (0.95, 0.095), (0.99, 0.099)] {
            let got = a.quantile(q).unwrap_or(0.0);
            assert!((got / want - 1.0).abs() < 0.05, "q{q}: {got} vs {want}");
        }
        assert_eq!(a.quantile(1.0), Some(0.1));
        assert!((a.mean().unwrap_or(0.0) - 0.05005).abs() < 1e-9);
    }

    #[test]
    fn zeros_count_and_rank_first() {
        let mut h = Hist::default();
        for _ in 0..9 {
            h.add(0.0);
        }
        h.add(0.2);
        assert_eq!(h.zero, 9);
        assert_eq!(h.quantile(0.5), Some(0.0));
        assert_eq!(h.quantile(0.99), Some(0.2));
    }
}
