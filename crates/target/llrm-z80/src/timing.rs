//! Bytes and T-states of the Z80 instruction forms, read from `timing/z80.csv`.
//!
//! The CSV is transcribed from Zilog's UM0080 instruction tables and checked
//! cell by cell against two assemblers (bytes) and `ucsim_z80` (T-states) by
//! `tests/timing/probe.py`; the cells where the simulator is the
//! instrument in error are named there; the Z180's forms are in `timing/z180.csv`, unsourced. No number lives in code.
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

fn parse(text: &'static str) -> HashMap<&'static str, Cost> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .map(|line| {
            let cells: Vec<&'static str> = line.split(',').collect();
            let number = |cell: &str| cell.parse::<u32>().unwrap_or_else(|_| panic!("timing csv: `{cell}` in `{line}`"));
            let cost = Cost {
                bytes: number(cells[1]),
                taken: number(cells[2]),
                not_taken: (!cells[3].is_empty()).then(|| number(cells[3])),
                cpu: cells[4],
            };
            (cells[0], cost)
        })
        .collect()
}

static TABLE: LazyLock<HashMap<&'static str, Cost>> = LazyLock::new(|| parse(include_str!("timing/z80.csv")));

/// The forms the Z180 adds, from their own file and source note.
static TABLE_Z180: LazyLock<HashMap<&'static str, Cost>> = LazyLock::new(|| parse(include_str!("timing/z180.csv")));

/// The cost of `form` on a plain Z80, written as in the CSV (`ld b;(hl)`: `;` stands for the operand comma).
pub fn cost(form: &str) -> Option<Cost> {
    TABLE.get(form).copied()
}

/// Every form of the Z80 table.
pub fn forms() -> impl Iterator<Item = (&'static str, Cost)> {
    TABLE.iter().map(|(form, cost)| (*form, *cost))
}

/// The cost of a form only the Z180 (and later) has.
pub fn cost_z180(form: &str) -> Option<Cost> {
    TABLE_Z180.get(form).copied()
}

/// Every form the Z180 adds.
pub fn forms_z180() -> impl Iterator<Item = (&'static str, Cost)> {
    TABLE_Z180.iter().map(|(form, cost)| (*form, *cost))
}
