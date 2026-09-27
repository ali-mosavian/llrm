//! Real-mode DOS as a target, what analyses ask of it through
//! `llrm_mir::target::Machine`.

use std::collections::BTreeSet;

use llrm_mir::target::{AddressForm, Machine, OperationCosts};

use crate::timings;

/// Real-mode DOS on one CPU: its prices and registers, and the built-in
/// description's foreign memory.
pub struct Dos {
    pub costs: OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
    pub address_forms: Vec<AddressForm>,
}

impl Default for Dos {
    /// On a 486, with the old profile's `register_capacity` and
    /// `call_register_capacity`.
    fn default() -> Self {
        let costs = costs("486");
        let address_forms = address_forms(&costs, 0);
        Self { costs, registers: 6, call_registers: 2, address_forms }
    }
}

impl Dos {
    /// On the CPU whose instruction forms cost `table` clocks, with
    /// `prefix` per operand-size prefix and `address_stall` more for an
    /// address-size one.
    pub fn priced(table: &[(String, i64)], prefix: i64, address_stall: i64, registers: i64, call_registers: i64) -> Self {
        let cost = |kind: &str| table.iter().find(|(one, _)| one == kind).unwrap_or_else(|| panic!("no price for {kind}")).1;
        let costs = operations(cost, prefix);
        let address_forms = address_forms(&costs, address_stall);
        Self { costs, registers, call_registers, address_forms }
    }
}

impl Machine for Dos {
    /// The description's foreign memory: `dos.toml` states it once.
    fn foreign_span(&self, selectors: (i64, i64), offsets: (i64, i64), width: i64) -> Option<(i64, i64)> {
        crate::machine::BUILT_IN.foreign_span(selectors, offsets, width)
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

    fn address_forms(&self) -> Vec<AddressForm> {
        self.address_forms.clone()
    }
}

/// The two indexed addresses real mode has. A word one is bx or bp plus si
/// or di, and bp is the frame: one register pairs with at most two others.
/// An address-size prefix buys any register as base or index, scaled by
/// 1, 2, 4 or 8, for `costs.prefix` and `address_stall` more a use and an
/// extension of the index to a dword.
pub fn address_forms(costs: &OperationCosts, address_stall: i64) -> Vec<AddressForm> {
    vec![
        AddressForm { partners: Some(2), ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("no fallback to disagree") },
        AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, costs.prefix + address_stall, costs.extend, true, None).expect("no fallback to disagree"),
    ]
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

    /// A word address is one of three registers and unscaled; a prefixed one
    /// scales any register by 1, 2, 4 or 8 for a clock a use on the 486.
    #[test]
    fn dos_states_its_address_forms() {
        let forms: Vec<_> = Dos::default().address_forms().iter().map(|one| (one.index_width, one.scales.iter().copied().collect::<Vec<_>>(), one.use_cost, one.address_registers())).collect();
        assert_eq!(forms, [(2, vec![1], 0, Some(3)), (4, vec![1, 2, 4, 8], 1, None)]);
    }
}
