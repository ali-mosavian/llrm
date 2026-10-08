//! The CPUs' timings, read from `timings.times` (the provenance of the figures
//! is its header). The cycles report prices the 486 and the later CPUs; the 386
//! has clocks for the passes and no cycle model.

use std::sync::LazyLock;

use llrm_support::hash::IndexMap;
use llrm_target::timings::{CpuTable, Timings};

/// The file, read once.
pub static TABLE: LazyLock<Timings> = LazyLock::new(|| Timings::parse(include_str!("timings.times")).expect("timings.times parses"));

pub const ARCHS: [&str; 7] = ["486", "P5", "P6", "K5", "K6", "K7", "Core"];

fn tables() -> Vec<&'static CpuTable> {
    ARCHS.iter().map(|name| TABLE.cpu(name).unwrap_or_else(|| panic!("timings.times has no {name}"))).collect()
}

fn scalars(of: impl Fn(&CpuTable) -> i64) -> [i64; 7] {
    let mut out = [0; 7];
    for (slot, table) in out.iter_mut().zip(tables()) {
        *slot = of(table);
    }
    out
}

/// A 16 bit write followed by a 32 bit read of the same register stalls the P6
/// while it merges the halves; Core recovers most of it with a merging uop.
pub static PARTIAL_STALL: LazyLock<[i64; 7]> = LazyLock::new(|| scalars(|table| table.partial_stall));

/// A 66h prefix on an instruction with an immediate changes its length and
/// stalls the decoder on P6 and Core.
pub static LCP_STALL: LazyLock<[i64; 7]> = LazyLock::new(|| scalars(|table| table.lcp_stall));

/// Instructions retired per cycle.
pub static ISSUE: LazyLock<[i64; 7]> = LazyLock::new(|| scalars(|table| table.issue));

/// Only the first two are in order.
pub static INORDER: LazyLock<[i64; 7]> = LazyLock::new(|| scalars(|table| i64::from(table.in_order)));

/// The 486 and P5 spend about a clock decoding each prefix, and every widened
/// instruction carries a 66h in 16-bit code.
pub static PREFIX: LazyLock<[i64; 7]> = LazyLock::new(|| scalars(|table| table.prefix));

/// Each form's clocks per CPU of `ARCHS`, where every one of them has a price.
fn per_cpu(of: impl Fn(&CpuTable) -> &Vec<(String, i64)>) -> IndexMap<&'static str, [i64; 7]> {
    let tables = tables();
    let mut out = IndexMap::default();
    for (form, _) in of(tables[0]) {
        let prices: Vec<Option<i64>> = tables.iter().map(|table| of(table).iter().find(|(one, _)| one == form).map(|(_, clocks)| *clocks)).collect();
        if prices.iter().all(Option::is_some) {
            let mut row = [0; 7];
            for (slot, price) in row.iter_mut().zip(prices) {
                *slot = price.expect("checked");
            }
            out.insert(&*Box::leak(form.clone().into_boxed_str()), row);
        }
    }
    out
}

pub static COST: LazyLock<IndexMap<&'static str, [i64; 7]>> = LazyLock::new(|| per_cpu(|table| &table.clocks));

pub static LATENCY: LazyLock<IndexMap<&'static str, [i64; 7]>> = LazyLock::new(|| per_cpu(|table| &table.latency));

#[cfg(test)]
mod tests {
    use super::*;

    /// Figures spot-checked against the tables they were taken from. The 486's LEA is 1 clock (Intel's 486 table, as in HelpPC 2.10's
    /// `LEA reg,mem 2+EA 3 2 1`: 286, 386, 486); the column held the 386's 2.
    #[test]
    fn the_table_has_the_figures_it_was_read_from() {
        assert_eq!(TABLE.cpus(), ["386", "486", "P5", "P6", "K5", "K6", "K7", "Core"]);
        assert_eq!((*PARTIAL_STALL, *LCP_STALL, *ISSUE, *INORDER, *PREFIX), ([0, 0, 7, 0, 1, 1, 2], [0, 0, 6, 0, 0, 0, 3], [1, 2, 3, 4, 3, 3, 4], [1, 1, 0, 0, 0, 0, 0], [1, 1, 0, 0, 0, 0, 0]));
        for (k, row) in [("pop_m", [6, 1, 4, 3, 3, 4, 4]), ("mul_r16", [13, 11, 4, 4, 3, 5, 3]), ("lea", [1, 1, 1, 1, 1, 1, 1]), ("x87_div_m", [73, 39, 38, 62, 58, 24, 30]), ("x87_control_store", [3, 2, 4, 6, 4, 1, 6])] {
            assert_eq!(COST[k], row, "COST[{k}]");
            assert_eq!(LATENCY[k], row, "LATENCY[{k}]");
        }
        assert_eq!(LATENCY["alu_rm"], [2, 2, 4, 3, 3, 3, 4]);
        assert_eq!(COST["alu_rm"], [2, 2, 1, 1, 1, 1, 1]);
    }

    /// The forms and the order they are listed in: the cycles report prints them so.
    #[test]
    fn the_cost_and_latency_tables_list_the_forms_they_did() {
        assert_eq!(COST.len(), 55);
        assert_eq!(LATENCY.len(), 55);
        assert_eq!(COST.keys().next(), Some(&"alu_rr"));
        assert_eq!(COST.keys().last(), Some(&"jcc_not_taken"));
        assert_eq!(LATENCY.keys().last(), Some(&"jcc_not_taken"));
    }
}
