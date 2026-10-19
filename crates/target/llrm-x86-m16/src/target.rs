//! Real-mode DOS as a target, what analyses ask of it through
//! `llrm_mir::target::Machine`.

use std::collections::BTreeSet;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, Machine, OperationCosts};

use crate::timings;

/// The registers a value may be placed in.
pub const GENERAL: [RegId; 6] = [RegId::EAX, RegId::EBX, RegId::ECX, RegId::EDX, RegId::ESI, RegId::EDI];

/// The registers a 16-bit address is encoded with, `[bx+si]`: a base is BX or
/// BP and an index SI or DI.
pub const ENCODABLE_BASES: [RegId; 2] = llrm_x86::addressing16::BASES;
pub const WORD_INDEXES: [RegId; 2] = llrm_x86::addressing16::INDEXES;

/// The frame register: no value is held in it.
pub const FRAME: RegId = RegId::BP;

/// The bases a value may be held in: the encodable ones but the frame's.
pub fn word_bases() -> Vec<RegId> {
    ENCODABLE_BASES.into_iter().filter(|&one| one != FRAME).collect()
}

/// Those a C callee keeps, as their word halves.
pub const PRESERVED: [(RegId, RegId); 2] = [(RegId::ESI, RegId::SI), (RegId::EDI, RegId::DI)];

/// How real-mode operations are priced from the instruction forms:
/// `opcosts.txt`.
pub static DESCRIPTION: std::sync::LazyLock<llrm_target::opcosts::Description> = std::sync::LazyLock::new(|| {
    llrm_target::opcosts::Description::parse(include_str!("opcosts.txt")).expect("opcosts.txt parses")
});

/// Real-mode DOS on one CPU: its prices and registers, and the built-in
/// description's foreign memory.
pub struct Dos {
    pub costs: OperationCosts,
    pub registers: i64,
    pub call_registers: i64,
    /// The floating values it holds in registers at once.
    pub float_registers: i64,
    pub address_forms: Vec<AddressForm>,
    /// Registers a far access takes for its selector.
    pub far_access: i64,
    /// What the description gives a function nothing outside the program
    /// reaches.
    pub private: Option<llrm_mir::target::PrivateConvention>,
    /// The conventions as the description states them: where arguments go and
    /// who removes them.
    pub calling: Option<&'static llrm_target::calling::Calling>,
}

/// The segment registers a selector is held in: ES, FS and GS, and DS where
/// no data is addressed through it, as the allocator takes it once those run
/// out.
pub const SEGMENT_REGISTERS: i64 = 4;

/// A far access sets a segment register from its selector, through a
/// general register.
pub const FAR_ACCESS: i64 = 1;

impl Default for Dos {
    /// On a 486.
    fn default() -> Self {
        let costs = costs("486");
        let address_forms = address_forms(&costs, 0);
        Self {
            costs,
            registers: GENERAL.len() as i64,
            call_registers: PRESERVED.len() as i64,
            float_registers: 8,
            address_forms,
            far_access: FAR_ACCESS,
            private: None,
            calling: None,
        }
    }
}

impl Dos {
    pub fn private(
        self,
        private: Option<llrm_mir::target::PrivateConvention>,
    ) -> Self {
        Self { private, ..self }
    }

    /// With the conventions the description states.
    pub fn calling(
        self,
        calling: Option<&'static llrm_target::calling::Calling>,
    ) -> Self {
        Self { calling, ..self }
    }

    /// On the CPU whose instruction forms cost `table` clocks, with
    /// `prefix` per operand-size prefix and `address_stall` more for an
    /// address-size one.
    pub fn priced(
        table: &[(String, i64)],
        prefix: i64,
        address_stall: i64,
        registers: i64,
        call_registers: i64,
        float_registers: i64,
    ) -> Self {
        let cost =
            |kind: &str| table.iter().find(|(one, _)| one == kind).unwrap_or_else(|| panic!("no price for {kind}")).1;
        let costs = DESCRIPTION.operations(&cost, prefix);
        let address_forms = address_forms(&costs, address_stall);
        Self {
            costs,
            registers,
            call_registers,
            float_registers,
            address_forms,
            far_access: FAR_ACCESS,
            private: None,
            calling: None,
        }
    }
}

impl Machine for Dos {
    /// LLVM prices 16-bit code with the i386 X86 model (`-code16`: the same
    /// subtarget, operand-size prefixes), and every CPU the target accepts
    /// reads the x87 tables as on m32 (`llrm_x86::tti`, T3).
    fn tti(&self) -> Option<std::rc::Rc<dyn llrm_mir::tti::Tti>> {
        let scales: std::collections::BTreeSet<i64> =
            self.address_forms.iter().flat_map(|form| form.scales.iter().copied()).collect();
        let scale_costs = scales
            .into_iter()
            .filter_map(|scale| {
                let cheapest =
                    self.address_forms.iter().filter(|form| form.scales.contains(&scale)).map(|form| form.use_cost);
                Some((scale, cheapest.min()?))
            })
            .collect();
        let real = llrm_x86::tti::RealMode { scale_costs, registers: self.registers as u32 };
        let layout = crate::layout();
        llrm_x86::tti::X86::described(layout.tti.as_deref()?, layout.llvm_cpu.as_deref()?)
            .map(|one| std::rc::Rc::new(one.in_real_mode(real)) as std::rc::Rc<dyn llrm_mir::tti::Tti>)
    }

    fn lowering(&self) -> Option<std::rc::Rc<dyn llrm_mir::lowering::Lowering>> {
        Some(std::rc::Rc::new(llrm_x86::lowering::X86))
    }

    fn private_convention(&self) -> Option<llrm_mir::target::PrivateConvention> {
        self.private.clone()
    }

    fn callee_pop(
        &self,
        convention: u32,
    ) -> Option<u32> {
        match self.calling {
            Some(calling) => calling.callee_pop(convention),
            None => (convention == 0).then_some(llrm_mir::opcode::FAST),
        }
    }

    fn stack_argument_bytes(
        &self,
        convention: u32,
        arguments: &[llrm_mir::target::Argument],
    ) -> Option<i64> {
        let found = self.calling?.by_number(convention)?;
        let kinds: Vec<_> = arguments.iter().map(|one| found.kind(*one, found.slot_bytes)).collect();
        Some(found.place(&kinds).stack_bytes)
    }

    fn spaces(&self) -> llrm_mir::spaces::Spaces {
        crate::spaces()
    }

    /// The description's foreign memory: `dos.toml` states it once.
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        crate::machine::BUILT_IN.foreign_span(selectors, offsets, width)
    }

    fn costs(&self) -> OperationCosts {
        self.costs.clone()
    }

    fn size_costs(&self) -> OperationCosts {
        // The by-pieces ratios are the CPU's, whichever table prices the rest.
        OperationCosts {
            pieces_set: self.costs.pieces_set,
            pieces_clear: self.costs.pieces_clear,
            pieces_set_size: self.costs.pieces_set_size,
            pieces_clear_size: self.costs.pieces_clear_size,
            ..DESCRIPTION.size_costs()
        }
    }

    fn registers(&self) -> i64 {
        self.registers
    }

    fn call_registers(&self) -> i64 {
        self.call_registers
    }

    fn float_registers(&self) -> i64 {
        self.float_registers
    }

    fn far_access_registers(&self) -> i64 {
        self.far_access
    }

    fn segment_registers(&self) -> i64 {
        SEGMENT_REGISTERS
    }

    fn two_address(&self) -> bool {
        true
    }

    fn address_registers(&self) -> i64 {
        (word_bases().len() + WORD_INDEXES.len()) as i64
    }

    fn address_forms(&self) -> Vec<AddressForm> {
        self.address_forms.clone()
    }

    /// The description's: `dos.toml` states when an access faults.
    fn load_may_trap(
        &self,
        width: u64,
        align: u64,
    ) -> bool {
        crate::machine::BUILT_IN.access_may_trap(width, align)
    }

    /// A far pointer (`p1`) whose offset is below one selector step: 64K
    /// from it, less that step's last byte, never carries.
    fn huge_window(&self) -> Option<(u32, i64)> {
        let step = 1_i64 << (16 - crate::machine::BUILT_IN.huge_shift()?);
        Some((crate::spaces().far, (1 << 16) - (step - 1)))
    }

    /// The description's: `dos.toml` states each device's reach.
    fn port_touches_memory(
        &self,
        ports: (i64, i64),
    ) -> bool {
        crate::machine::BUILT_IN.port_memory(ports) != crate::machine::PortMemory::None
    }
}

/// The two indexed addresses real mode has. A word one is bx or bp plus si
/// or di, and bp is the frame: one register pairs with at most two others.
/// An address-size prefix buys any register as base or index, scaled by
/// 1, 2, 4 or 8, for `costs.prefix` and `address_stall` more a use and an
/// extension of the index to a dword.
pub fn address_forms(
    costs: &OperationCosts,
    address_stall: i64,
) -> Vec<AddressForm> {
    vec![
        // BX is the base and SI and DI the indices: BP is the frame's.
        AddressForm {
            partners: Some(2),
            bases: Some(1),
            indices: Some(2),
            ..AddressForm::new(2, BTreeSet::from([1]), 0, 0, 0, false, None).expect("no fallback to disagree")
        },
        AddressForm::new(4, BTreeSet::from([1, 2, 4, 8]), 1, costs.prefix + address_stall, costs.extend, true, None)
            .expect("no fallback to disagree"),
    ]
}

/// `arch`'s (one of `timings::ARCHS`) price of each operation, as the
/// instructions lowering picks for it.
pub fn costs(arch: &str) -> OperationCosts {
    let at = timings::ARCHS.iter().position(|one| *one == arch).expect("a listed arch");
    OperationCosts {
        pieces_set: timings::SET_RATIO[at],
        pieces_clear: timings::CLEAR_RATIO[at],
        pieces_set_size: timings::SET_RATIO_SIZE[at],
        pieces_clear_size: timings::CLEAR_RATIO_SIZE[at],
        jump_table: timings::JUMP_TABLE[at],
        cache_line: timings::CACHE_LINE[at],
        ..DESCRIPTION.operations(&|kind| timings::COST[kind][at], timings::PREFIX[at])
    }
}

/// Bytes of `op r, r` on `width`-byte registers; a dword runs under the 66h
/// prefix.
pub fn register_bytes(width: i64) -> i64 {
    llrm_x86::encoding::register_bytes(width, 2)
}

/// Bytes of a shift of a `width`-byte register by `count`.
pub fn shift_bytes(
    count: i64,
    width: i64,
) -> i64 {
    llrm_x86::encoding::shift_bytes(count, width, 2)
}

/// Bytes of `imul r, r, number` on `width`-byte registers.
pub fn imul_immediate_bytes(
    number: i64,
    width: i64,
) -> i64 {
    llrm_x86::encoding::imul_immediate_bytes(number, width, 2)
}

#[cfg(test)]
mod tests {
    use llrm_mir::target::Machine;

    use super::Dos;

    /// LSR reads the addressing hooks of the model: real mode's `[bx+si]` is
    /// free, a scaled or any-register address pays the prefix and its stall,
    /// and a value sits in one of six registers. The 32-bit mode's model made
    /// every second register cost 1 and counted eight.
    #[test]
    fn the_model_reads_real_modes_addressing_and_registers() {
        let costs = super::costs("486");
        let dos = Dos { address_forms: super::address_forms(&costs, 1), costs, ..Dos::default() };
        let tti = dos.tti().expect("m16 states its cost model");
        let cost = |scale| tti.scaling_factor_cost(false, 0, true, scale).value;
        assert_eq!((cost(0), cost(1)), (0, 0), "plain and bx+si are free");
        assert_eq!((cost(2), cost(4), cost(8)), (2, 2, 2), "the prefix (1) and the stall (1)");
        assert!(tti.is_legal_addressing_mode(false, 0, true, 8));
        assert_eq!(tti.number_of_gpr_registers(), 6);
    }

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
        let forms: Vec<_> = Dos::default()
            .address_forms()
            .iter()
            .map(|one| {
                (one.index_width, one.scales.iter().copied().collect::<Vec<_>>(), one.use_cost, one.address_registers())
            })
            .collect();
        assert_eq!(forms, [(2, vec![1], 0, Some(3)), (4, vec![1, 2, 4, 8], 1, None)]);
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::{imul_immediate_bytes, register_bytes, shift_bytes};

    /// The coarse table and the exact helpers agree on a word, so `size_costs`
    /// read what isel's -Os pricing reads.
    #[test]
    fn the_size_table_and_the_encodings_agree_on_a_word() {
        // Twice the instruction: an operation is the instruction and what goes
        // around it.
        let doubled = |kind: &str| super::DESCRIPTION.bytes(kind).expect("opcosts.txt has the bytes");
        assert_eq!(
            (doubled("alu_rr"), doubled("mov_rr"), doubled("shift_ri")),
            (2 * register_bytes(2), 2 * register_bytes(2), 2 * shift_bytes(2, 2))
        );
    }

    /// The byte prices MIR decides inlining and the calling convention by: a
    /// call is its 5 bytes and each argument word 2 more (push and
    /// cleanup), a cleanup by pop is 1 a word against `add sp` 3, and `ret
    /// N` 2 over `ret`; any other operation is its instruction's bytes
    /// twice, the instructions an operation becomes. In clocks an argument is
    /// its store and load.
    #[test]
    fn the_size_costs_are_the_bytes_of_the_code_an_operation_becomes() {
        use llrm_mir::target::Machine;
        let dos = super::Dos::default();
        let sized = dos.size_costs();
        assert_eq!((sized.call, sized.argument, sized.pop, sized.adjust, sized.return_pops), (5, 2, 1, 3, 2));
        let bytes = |kind: &str| super::DESCRIPTION.bytes(kind).expect("opcosts.txt has the bytes");
        assert_eq!(
            (sized.load, sized.store, sized.add, sized.branch),
            (bytes("mov_rm"), bytes("mov_mr"), bytes("alu_rr"), bytes("jcc"))
        );
        let clocks = dos.costs();
        assert_eq!(clocks.argument, clocks.load + clocks.store);
    }

    /// A dword takes 66h, a shift by one is D1, and `imul` takes a byte
    /// immediate only where it fits.
    #[test]
    fn a_dword_form_has_the_operand_size_prefix() {
        assert_eq!((register_bytes(2), register_bytes(4)), (2, 3));
        assert_eq!((shift_bytes(1, 4), shift_bytes(3, 4), shift_bytes(1, 2)), (3, 4, 2));
        assert_eq!((imul_immediate_bytes(6, 4), imul_immediate_bytes(446, 4), imul_immediate_bytes(446, 2)), (4, 7, 4));
    }
}
