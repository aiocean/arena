//! Stored cooldown uses (design, 17.8; PoE's "+1 cooldown use"): a skill holds `1 +` [`Stat::ExtraCooldownUse`]
//! uses, all stored at the start. Each cast spends one; while fewer than full, one recharges at a time over the
//! skill's cooldown after `CooldownRecovery` (`build::Built::cooldown`). The fight ticks it with its own clock (Ngạt
//! runs cooldowns at `ailment::AIL.choke_cooldown`: pass `dt` times that).

use crate::build::Built;
use crate::stat::{Stat, Table};

/// The most uses a skill can store.
pub const MAX_USES: u8 = 16;

/// A skill's stored uses: at most `max`, `stored` now, `recharge_left` seconds until the next comes back (0 when
/// full), each taking `cooldown`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Uses {
    pub max: u8,
    pub stored: u8,
    pub recharge_left: f64,
    pub cooldown: f64,
}

impl Uses {
    /// `1 + extra` uses (at most [`MAX_USES`]), all stored, each recharging over `cooldown` seconds.
    pub fn new(cooldown: f64, extra: u8) -> Uses {
        let max = extra.saturating_add(1).min(MAX_USES);
        Uses {
            max,
            stored: max,
            recharge_left: 0.0,
            cooldown: cooldown.max(0.0),
        }
    }

    /// The extra uses a cast's table gives (`Stat::ExtraCooldownUse`, rounded, never below zero).
    pub fn extra_of(t: &Table) -> u8 {
        libm::round(
            t.get(Stat::ExtraCooldownUse, 0.0)
                .clamp(0.0, f64::from(MAX_USES - 1)),
        ) as u8
    }

    /// A built skill's uses: its cooldown after recovery, its extra uses from its table.
    pub fn of(b: &Built) -> Uses {
        Uses::new(b.cooldown, Uses::extra_of(&b.table))
    }

    /// Spends a use if one is stored; the recharge starts if none was running. Whether it could.
    pub fn try_use(&mut self) -> bool {
        if self.stored == 0 {
            return false;
        }
        if self.stored == self.max {
            self.recharge_left = self.cooldown;
        }
        self.stored -= 1;
        if self.cooldown <= 0.0 {
            self.stored = self.max;
            self.recharge_left = 0.0;
        }
        true
    }

    /// Time passes: uses come back one at a time, the rest of `dt` running into the next.
    pub fn tick(&mut self, dt: f64) {
        let mut dt = dt.max(0.0);
        while self.stored < self.max {
            if self.recharge_left > dt {
                self.recharge_left -= dt;
                return;
            }
            dt -= self.recharge_left;
            self.stored += 1;
            self.recharge_left = if self.stored < self.max {
                self.cooldown
            } else {
                0.0
            };
            if self.cooldown <= 0.0 {
                self.stored = self.max;
                self.recharge_left = 0.0;
            }
        }
    }

    /// Whether a use is stored.
    pub fn ready(&self) -> bool {
        self.stored > 0
    }
}
