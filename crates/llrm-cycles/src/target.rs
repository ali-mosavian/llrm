//! Real-mode DOS as a target, what analyses ask of it through
//! `llrm_mir::target::Machine`.

use llrm_mir::target::{Machine, OperationCosts};

use crate::timings;

/// Real-mode DOS on one CPU: its prices and registers, and its foreign
/// memory -- VGA and text video memory, and the ROMs above.
pub struct Dos {
    pub costs: OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
}

impl Default for Dos {
    /// On a 486, with the old profile's `register_capacity` and
    /// `call_register_capacity`.
    fn default() -> Self {
        Self { costs: costs("486"), registers: 6, call_registers: 2 }
    }
}

impl Dos {
    /// On the CPU whose instruction forms cost `table` clocks, with
    /// `prefix` per operand-size prefix.
    pub fn priced(table: &[(String, i64)], prefix: i64, registers: i64, call_registers: i64) -> Self {
        let cost = |kind: &str| table.iter().find(|(one, _)| one == kind).unwrap_or_else(|| panic!("no price for {kind}")).1;
        Self { costs: operations(cost, prefix), registers, call_registers }
    }
}

impl Machine for Dos {
    fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)> {
        let word = |(low, high): (i64, i64)| 0 <= low && low <= high && high <= 0xFFFF;
        if !word(selectors) || !word(offsets) {
            return None;
        }
        let (start, end) = (selectors.0 * 16 + offsets.0, selectors.1 * 16 + offsets.1 + width);
        let mut reached = start;
        for (from, to) in [(0xA0000, 0xC0000), (0xC0000, 0x10_0000)] {
            if from <= reached && reached < to {
                reached = to;
            }
        }
        (end <= reached).then_some((start, end))
    }

    fn costs(&self) -> OperationCosts {
        self.costs.clone()
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }
}

/// `arch`'s (one of `timings::ARCHS`) price of each operation, as the
/// instructions lowering picks for it.
pub fn costs(arch: &str) -> OperationCosts {
    let at = timings::ARCHS.iter().position(|one| *one == arch).expect("a listed arch");
    operations(|kind| timings::COST[kind][at], timings::PREFIX[at])
}

/// The price of each operation, as the instructions lowering picks for it
/// cost `cost(kind)` clocks.
fn operations(cost: impl Fn(&str) -> i64, prefix: i64) -> OperationCosts {
    OperationCosts {
        add: cost("alu_rr"),
        multiply: cost("mul_r16"),
        divide: cost("div_r16"),
        shift: cost("shift_ri"),
        address: cost("lea"),
        load: cost("mov_rm"),
        store: cost("mov_mr"),
        memory_update: cost("alu_mr"),
        branch: cost("jcc"),
        prefix,
        r#move: cost("mov_rr"),
        call: cost("call_far"),
        return_: cost("ret_far"),
        float_add: cost("x87_add"),
        float_multiply: cost("x87_mul"),
        float_divide: cost("x87_div"),
        float_load: cost("x87_load"),
        float_store: cost("x87_store"),
        extend: cost("movzx"),
        // Saving ES, loading it with the cells' segment, and setting the
        // value and count before `rep stos`; then restoring ES.
        fill: cost("rep_stos") + 2 * cost("push_r") + 2 * cost("pop_seg") + 2 * cost("mov_ri"),
        fill_cell: cost("rep_stos_cell"),
    }
}

#[cfg(test)]
mod tests {
    use llrm_mir::target::Machine;

    use super::Dos;

    /// Dos prices at the 486's clocks: a 16-bit divide and multiply, and a
    /// fill's setup around `rep stos`.
    #[test]
    fn dos_prices_the_486() {
        let costs = Dos::default().costs();
        assert_eq!((costs.divide, costs.multiply, costs.prefix, costs.fill_cell), (24, 13, 1, 4));
    }
}
