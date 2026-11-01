//! Bytes and T-states of the Z80 instruction forms, read from `timing/z80.csv`.
//!
//! The CSV is transcribed from Zilog's UM0080 instruction tables and checked
//! cell by cell against two assemblers (bytes) and `ucsim_z80` (T-states) by
//! `tests/timing/probe.py`; the four cells where the simulator is the
//! instrument in error are named there. No number lives in code.
//!
//! DEVIATE: no wait states and no memory contention (Spectrum, MSX). Reason:
//! they belong to the machine, not the CPU. Exit: a per-machine surcharge when
//! a machine target needs it.

use std::collections::HashMap;
use std::sync::LazyLock;

/// What one instruction form costs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cost {
    pub bytes: u32,
    /// T-states; for a conditional form, those of the taken path (a repeating
    /// `ldir` pass).
    pub taken: u32,
    /// The not-taken path (the last `ldir` pass), where the form has two.
    pub not_taken: Option<u32>,
    /// The first CPU that has the form: `z80` or `z180`.
    pub cpu: &'static str,
}

static TABLE: LazyLock<HashMap<&'static str, Cost>> = LazyLock::new(|| {
    include_str!("timing/z80.csv")
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let cells: Vec<&'static str> = line.split(',').collect();
            let number = |cell: &str| cell.parse::<u32>().unwrap_or_else(|_| panic!("z80.csv: `{cell}` in `{line}`"));
            let cost = Cost {
                bytes: number(cells[1]),
                taken: number(cells[2]),
                not_taken: (!cells[3].is_empty()).then(|| number(cells[3])),
                cpu: cells[4],
            };
            (cells[0], cost)
        })
        .collect()
});

/// The cost of `form`, written as in the CSV (`ld b;(hl)`: `;` stands for the
/// operand comma).
pub fn cost(form: &str) -> Option<Cost> {
    TABLE.get(form).copied()
}

/// Every form in the table.
pub fn forms() -> impl Iterator<Item = (&'static str, Cost)> {
    TABLE.iter().map(|(form, cost)| (*form, *cost))
}
