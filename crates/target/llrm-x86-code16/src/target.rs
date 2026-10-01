//! Real-mode DOS as a target, what analyses ask of it through
//! `llrm_mir::target::Machine`.

use std::collections::BTreeSet;

use iced_x86::Register;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};

use crate::timings;

/// The registers a value may be placed in.
pub const GENERAL: [Register; 6] = [Register::EAX, Register::EBX, Register::ECX, Register::EDX, Register::ESI, Register::EDI];

/// Those a C callee keeps, as their word halves.
pub const PRESERVED: [(Register, Register); 2] = [(Register::ESI, Register::SI), (Register::EDI, Register::DI)];

/// Real-mode DOS on one CPU: its prices and registers, and the built-in
/// description's foreign memory.
pub struct Dos {
    pub costs: OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
    pub address_forms: Vec<AddressForm>,
    /// Registers a far access takes for its selector.
    pub far_access: i64,
}

/// The segment registers a selector is held in: ES, FS and GS, and DS where
/// no data is addressed through it, as the allocator takes it once those run out.
pub const SEGMENT_REGISTERS: i64 = 4;

/// A far access sets a segment register from its selector, through a
/// general register.
pub const FAR_ACCESS: i64 = 1;

impl Default for Dos {
    /// On a 486.
    fn default() -> Self {
        let costs = costs("486");
        let address_forms = address_forms(&costs, 0);
        Self { costs, registers: GENERAL.len() as i64, call_registers: PRESERVED.len() as i64, address_forms, far_access: FAR_ACCESS }
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
        Self { costs, registers, call_registers, address_forms, far_access: FAR_ACCESS }
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

    fn size_costs(&self) -> OperationCosts {
        operations(bytes, 1)
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }

    fn far_access_registers(&self) -> i64 {
        self.far_access
    }

    fn segment_registers(&self) -> i64 {
        SEGMENT_REGISTERS
    }

    fn address_forms(&self) -> Vec<AddressForm> {
        self.address_forms.clone()
    }

    /// The description's: `dos.toml` states when an access faults.
    fn load_may_trap(&self, width: u64, align: u64) -> bool {
        crate::machine::BUILT_IN.access_may_trap(width, align)
    }

    /// The description's: `dos.toml` states each device's reach.
    fn port_touches_memory(&self, ports: (i64, i64)) -> bool {
        crate::machine::BUILT_IN.port_memory(ports) != crate::machine::PortMemory::None
    }
}

/// The two indexed addresses real mode has. A word one is bx or bp plus si
/// or di, and bp is the frame: one register pairs with at most two others.
/// An address-size prefix buys any register as base or index, scaled by
/// 1, 2, 4 or 8, for `costs.prefix` and `address_stall` more a use and an
/// extension of the index to a dword.
pub fn address_forms(costs: &OperationCosts, address_stall: i64) -> Vec<AddressForm> {
    vec![
        // BX is the base and SI and DI the indices: BP is the frame's.
        AddressForm { partners: Some(2), bases: Some(1), indices: Some(2), ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("no fallback to disagree") },
        AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, costs.prefix + address_stall, costs.extend, true, None).expect("no fallback to disagree"),
    ]
}

/// `arch`'s (one of `timings::ARCHS`) price of each operation, as the
/// instructions lowering picks for it.
pub fn costs(arch: &str) -> OperationCosts {
    let at = timings::ARCHS.iter().position(|one| *one == arch).expect("a listed arch");
    operations(|kind| timings::COST[kind][at], timings::PREFIX[at])
}

/// Bytes of the instruction lowering picks for each kind of operation, as
/// real mode encodes it: a register form is 2, one with a displacement 3. A
/// far call is 5, and its pushes and cleanup 3 more.
fn bytes(kind: &str) -> i64 {
    match kind {
        "alu_rr" | "mov_rr" | "jcc" | "rep_stos" => 2,
        "ret_far" | "push_r" | "pop_seg" => 1,
        "call_far" => 8,
        "rep_stos_cell" => 0,
        _ => 3,
    }
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
        carry: llrm_mir::target::carry_cost(cost("movzx"), cost("alu_rr"), cost("shift_ri"), cost("mov_rr")),
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
