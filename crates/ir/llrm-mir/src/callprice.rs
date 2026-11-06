//! What a call costs and what a byte of code is worth, from the target's own
//! tables: the one price inlining, self-copying and call-lowering decisions
//! share.

use crate::target::OperationCosts;

/// The sheet's price of a call with `arguments` words, and the target's rate
/// between a byte of code and a clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallPrice {
    /// Clocks a call, its return and its argument words take, which
    /// inlining the callee saves.
    pub clocks: i64,
    /// Bytes of code the same call, return and argument words are.
    pub bytes: i64,
    /// What a byte is worth in thousandths of a clock
    /// (`Tti::speed_milliclocks_per_byte`).
    pub milliclocks_per_byte: i64,
}

impl CallPrice {
    pub fn new(
        clock_costs: &OperationCosts,
        size_costs: &OperationCosts,
        arguments: usize,
        milliclocks_per_byte: i64,
    ) -> Self {
        Self {
            clocks: clock_costs.call_overhead(arguments),
            bytes: size_costs.call_overhead(arguments),
            milliclocks_per_byte,
        }
    }

    /// Whether `clocks` saved are worth `bytes` added, at `margin` times the
    /// bytes' price. A target that does not trade bytes for clocks (rate 0)
    /// pays nothing for any growth.
    pub fn pays(
        &self,
        clocks: f64,
        bytes: f64,
        margin: f64,
    ) -> bool {
        self.milliclocks_per_byte > 0 && bytes * self.milliclocks_per_byte as f64 * margin <= clocks * 1000.0
    }
}

impl OperationCosts {
    /// What a call costs beyond its own instruction, which inlining saves: the
    /// return, and each argument pushed by the caller and read back by the
    /// callee.
    pub fn call_overhead(
        &self,
        arguments: usize,
    ) -> i64 {
        self.call + self.return_ + arguments as i64 * self.argument
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn price(rate: i64) -> CallPrice {
        CallPrice { clocks: 35, bytes: 10, milliclocks_per_byte: rate }
    }

    /// Growth is paid for when its clocks reach the bytes' price; a margin
    /// scales the price, and a target with no byte rate pays for none.
    #[test]
    fn growth_is_paid_for_by_clocks_at_the_byte_rate() {
        assert!(price(500).pays(31.0, 62.0, 1.0));
        assert!(!price(500).pays(30.0, 62.0, 1.0));
        assert!(!price(500).pays(31.0, 62.0, 4.0));
        assert!(!price(0).pays(1e9, 1.0, 1.0));
    }
}
