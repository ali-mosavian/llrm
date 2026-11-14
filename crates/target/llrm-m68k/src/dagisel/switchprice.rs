//! What a jump table costs against the compare chain, on m68k's own tables:
//! the two tuning limits switch lowering reads.
//!
//! The pin's `isSuitableForJumpTable` takes a range of cases when
//! `NumCases * 100 >= Range * getMinimumJumpTableDensity(OptForSize)` (10%,
//! 40% at `-Os`) and `findJumpTables` needs `getMinimumJumpTableEntries` cases
//! (4). Both are target hooks; M68k states neither. DEVIATE M68K-JT-PRICE: here
//! they are derived per CPU and level from the instructions a table and a
//! chain become, as [`super::mulprice`] prices a multiply, with the level's
//! [`Price`]. A 4-case switch over a range of 28 was a 28-entry table: +90 to
//! +140 B on the jpeg_* cells, no faster than four compares.
//!
//! - The density is a table slot (a `.long`, 4 B) against what one case costs
//!   in a chain (`cmpi.l #imm, %d0` and `bcc`): bytes at `-Os` and `-O1`, bytes
//!   plus the clocks a case adds to the chain's walk at `-O2`/`-O3`.
//! - The entries are the fewest cases of a dense switch for which the table's
//!   dispatch (bias, bound check, scale, `lea`, indexed `move.l`, `jmp`) plus
//!   its slots beats the chain's compares by the level's price.
//!
//! Owner m68k. Exit 2026-12-20, with the price from the selected instructions
//! (M68K-MUL-PRICE).
//!
//! Complexity: O(1) per function.

use super::hooks::Cpu;
use crate::timing::{Price, byte_milliclocks, cost};

/// The two limits `BuilderTarget` carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// `getMinimumJumpTableEntries`.
    pub min_entries: usize,
    /// `getMinimumJumpTableDensity`, percent.
    pub min_density: u64,
}

/// `(bytes, clocks)` of `text`, which is `bytes` long. A conditional branch is
/// priced at the mean of falling through and taking it.
fn one(
    cpu: Cpu,
    text: &str,
    bytes: usize,
) -> (usize, u32) {
    let clocks = |taken| cost(cpu, text, bytes as u32, taken).map_or(0, |cost| cost.clocks);
    (bytes, (clocks(false) + clocks(true)).div_ceil(2))
}

fn sum(parts: impl IntoIterator<Item = (usize, u32)>) -> (usize, u32) {
    parts.into_iter().fold((0, 0), |(b, c), (pb, pc)| (b + pb, c + pc))
}

/// One compare of the chain: `cmpi.l #imm, %d0` and the branch on it.
fn compare(cpu: Cpu) -> (usize, u32) {
    sum([one(cpu, "cmpi.l #200, %d0", 6), one(cpu, "beq .L1", 2)])
}

/// The table's dispatch (M68K-WORD-TABLE, [`crate::wordtable`]): the bias, the
/// bound check, the index doubled, the word load from the table and the jump
/// through it. The same from the 68000 on: no scaled index to save a step.
fn dispatch(cpu: Cpu) -> (usize, u32) {
    sum([
        one(cpu, "moveq #64, %d1", 2),
        one(cpu, "add.l %d1, %d0", 2),
        one(cpu, "moveq #27, %d1", 2),
        one(cpu, "cmp.l %d1, %d0", 2),
        one(cpu, "bhi .L1", 2),
        one(cpu, "add.l %d0, %d0", 2),
        one(cpu, "move.w (.LJTI,%pc,%d0), %d0", 4),
        one(cpu, "jmp (.LJTI,%pc,%d0)", 4),
    ])
}

/// A table slot: a `.word`.
const SLOT: usize = 2;

/// The slot the density limit counts: the `.long` of the pin's table, kept so
/// a sparse switch stays a chain (the jpeg cells: a 14% table was +90..140 B
/// and no faster). The word slot lowers the limit of entries alone; the
/// density is retuned by its own A/B.
const DENSITY_SLOT: usize = 4;

/// The compares a balanced tree of `cases` makes to find one (the pin's
/// `splitWorkItem`).
fn walk(cases: usize) -> u32 {
    usize::BITS - cases.leading_zeros()
}

/// A dense switch of `cases`: `(table, chain)`, each `(bytes, clocks)`.
fn dense(
    cpu: Cpu,
    cases: usize,
) -> ((usize, u32), (usize, u32)) {
    let (dispatch_bytes, dispatch_clocks) = dispatch(cpu);
    let (compare_bytes, compare_clocks) = compare(cpu);
    ((dispatch_bytes + SLOT * cases, dispatch_clocks), (compare_bytes * cases, compare_clocks * walk(cases)))
}

/// The limits for `cpu` at `price`.
pub fn limits(
    cpu: Cpu,
    price: Price,
) -> Limits {
    let min_entries = (2..=64)
        .find(|&cases| {
            let (table, chain) = dense(cpu, cases);
            price.beats(cpu, table, chain)
        })
        .unwrap_or(64);
    // what one case adds to the chain, in the price's unit: bytes, and at a
    // rate the clocks beyond the table's, over a switch twice the entries
    let (compare_bytes, _) = compare(cpu);
    let case = match price {
        Price::Size | Price::Strict => compare_bytes as i64 * 1000,
        Price::Rate => {
            let wide = 2 * min_entries;
            let (table, chain) = dense(cpu, wide);
            let saved = i64::from(chain.1) - i64::from(table.1);
            compare_bytes as i64 * byte_milliclocks(cpu) + 1000 * saved.max(0) / wide as i64
        }
    };
    let slot =
        if price == Price::Rate { DENSITY_SLOT as i64 * byte_milliclocks(cpu) } else { DENSITY_SLOT as i64 * 1000 };
    Limits { min_entries, min_density: (100 * slot + case - 1).div_euclid(case).clamp(1, 100) as u64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table needs enough cases to pay for its dispatch.
    #[test]
    fn the_entries_are_what_the_dispatch_costs() {
        for price in [Price::Size, Price::Strict, Price::Rate] {
            let old = limits(Cpu::M68000, price).min_entries;
            let new = limits(Cpu::M68020, price).min_entries;
            assert!((4..=12).contains(&old) && (4..=12).contains(&new), "{price:?}: {old} {new}");
        }
    }

    /// A density slot is 4 B against a case's 8: half at `-Os`, less where the
    /// chain's clocks count.
    #[test]
    fn the_density_is_a_slot_against_a_case() {
        for cpu in [Cpu::M68000, Cpu::M68020] {
            assert_eq!(limits(cpu, Price::Size).min_density, 50);
            let rate = limits(cpu, Price::Rate).min_density;
            assert!((10..50).contains(&rate), "{cpu:?}: {rate}");
        }
    }
}
