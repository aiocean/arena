//! The simulator's dice: SplitMix64, small, fast and the same bits on every machine. Every roll of a fight comes from
//! one of these, seeded from the match seed, so a match replays from (seed, loadout A, loadout B).

/// A seeded SplitMix64 stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

/// SplitMix64's increment (the golden ratio in 64 bits).
const GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;

impl Rng {
    /// A stream from `seed`.
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// A stream from `seed` and a salt, so two streams of one seed (the fight's dice, each seat's brain) differ.
    pub fn salted(seed: u64, salt: u64) -> Rng {
        let mut r = Rng::new(seed ^ salt.wrapping_mul(GAMMA).rotate_left(17));
        r.next_u64();
        r
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number in [0, 1), from the top 53 bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// A whole number in `0..n` (0 when `n` is 0).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((self.next_f64() * n as f64) as usize).min(n - 1)
    }
}
