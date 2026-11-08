//! The pin's FastISel opcode rows, by CPU table.
//!
//! # How it works
//! `tools/dagisel/regen.sh` writes `generated/fastisel_<cpu>.rs` from the pin's
//! `X86GenFastISel.inc`: the arm of each `fastEmit_*` function the CPU takes.
//! [`lookup`] binary-searches one of those tables.
//!
//! # Used by
//! The -O0 FastISel of `llrm-codegen`.

use crate::generated::{fastisel_i486, fastisel_m16, fastisel_p5, fastisel_x64};

/// A generated table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    I486,
    P5,
    M16,
    X64,
}

type Row = (&'static str, &'static str, &'static str, &'static str, u8, &'static str, &'static str);

impl Table {
    fn rows(self) -> &'static [Row] {
        match self {
            Table::I486 => fastisel_i486::ROWS,
            Table::P5 => fastisel_p5::ROWS,
            Table::M16 => fastisel_m16::ROWS,
            Table::X64 => fastisel_x64::ROWS,
        }
    }
}

/// The `(opcode, register class)` the pin's
/// `fastEmit_<namespace>_<op>_MVT_<vt>…` takes: `ret` is empty unless the
/// function is named with a result type; `shape` is [`Shape`](#) code 0 `r`, 1
/// `rr`, 2 `ri`, 3 `ri` with a predicate, 4 `i`.
pub fn lookup(
    table: Table,
    namespace: &str,
    op: &str,
    vt: &str,
    ret: &str,
    shape: u8,
) -> Option<(&'static str, &'static str)> {
    let rows = table.rows();
    let at =
        rows.binary_search_by(|row| (row.0, row.1, row.2, row.3, row.4).cmp(&(namespace, op, vt, ret, shape))).ok()?;
    Some((rows[at].5, rows[at].6))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FastISel opcodes were spelled by hand (`format!("{one}32rr")`); the
    /// pin's own rows say the same for the five ALU operations on `i32`.
    #[test]
    fn the_pins_alu_rows_are_the_hand_spelled_opcodes() {
        for table in [Table::I486, Table::P5, Table::X64] {
            for (op, name) in [("ADD", "ADD"), ("SUB", "SUB"), ("AND", "AND"), ("OR", "OR"), ("XOR", "XOR")] {
                assert_eq!(
                    lookup(table, "ISD", op, "i32", "", 1),
                    Some((&*format!("{name}32rr").leak(), "GR32")),
                    "{table:?} {op} rr"
                );
                assert_eq!(
                    lookup(table, "ISD", op, "i32", "", 2),
                    Some((&*format!("{name}32ri").leak(), "GR32")),
                    "{table:?} {op} ri"
                );
            }
        }
    }

    /// A binary search over an unsorted table misses rows silently: every table
    /// is in tuple-key order, not only in the order its lines sort as text.
    #[test]
    fn every_generated_table_is_in_key_order() {
        for table in [Table::I486, Table::P5, Table::M16, Table::X64] {
            let keys: Vec<_> = table.rows().iter().map(|r| (r.0, r.1, r.2, r.3, r.4)).collect();
            assert!(keys.windows(2).all(|w| w[0] < w[1]), "{table:?} is not sorted by key");
        }
    }
}
