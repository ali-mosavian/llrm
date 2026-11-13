//! The budgets the mid-end's passes read: [`Limits`].
//!
//! The GCC-model `admitted` (is a complete copy of a counted loop worth it) was
//! deleted with `Options::peel`, `Limits::grows` and the trip and operation
//! caps only it read: nothing called it outside its tests, and the translated
//! `LoopUnrollPass` decides the copies.

/// The copy budgets and the price of a code byte the pipeline's passes read.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Limits {
    /// 100, and -O3's 200 (LLVM's 300 over 150): -O3 and -Omax are told by it.
    pub target_percent: i64,
    /// Clocks an inline that grows the code must save for each byte it adds:
    /// `--clocks-per-byte`.
    pub milliclocks_per_byte: i64,
}

impl Default for Limits {
    fn default() -> Self {
        Self { target_percent: 0, milliclocks_per_byte: 16_000 }
    }
}
