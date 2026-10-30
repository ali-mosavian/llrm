//! The target-generic layer: what a target description is, owned by no ISA.
//! A target crate supplies the data; the passes read the type.

pub mod addressing;
pub mod calling;
pub mod instr;
pub mod isa;
pub mod layout;
pub mod machine;
pub mod object;
pub mod opcosts;
pub mod os;
#[cfg(test)]
mod os_tests;
pub mod pressure;
pub mod registers;
pub mod routines;
pub mod runtime;
pub mod timings;
pub mod tune;

use std::rc::Rc;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use machine::Machine;

/// The registers a segmented target's string operation names: the count, the
/// offsets it reads and writes through, and the segment its source defaults
/// to.
#[derive(Clone, Copy, Debug)]
pub struct StringRegisters {
    pub count: RegId,
    pub source: RegId,
    pub destination: RegId,
    pub source_segment: RegId,
    /// The segment a destination is written through, which a string move
    /// leaves loaded with the source's.
    pub destination_segment: RegId,
    /// A data register a tail's byte count rides in.
    pub tail: RegId,
}

/// A flat platform's foreign linear ranges, read when a constant address is
/// first asked of: parsing a platform's description is not paid by a compile
/// that never asks. Profiles are equal by their prices, so these compare equal.
#[derive(Clone, Copy)]
pub struct Foreign(pub fn() -> &'static [(i64, i64)]);

impl Foreign {
    /// No ranges: a target that is not flat, or whose platform states none.
    pub fn none() -> Self {
        Self(|| &[])
    }
}

impl std::fmt::Debug for Foreign {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str("Foreign")
    }
}

impl PartialEq for Foreign {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for Foreign {}

impl std::hash::Hash for Foreign {
    fn hash<H: std::hash::Hasher>(
        &self,
        _: &mut H,
    ) {
    }
}

/// A platform's port table, read when a port is first asked of, as `Foreign`
/// is: what an access to a port may do to memory is the platform's (the PC's
/// devices are the same under a 16-bit and a flat extender), so every target
/// built from a description answers it. Profiles are equal by their prices, so
/// these compare equal.
#[derive(Clone, Copy)]
pub struct PortTable(pub fn() -> &'static [machine::Port]);

impl PortTable {
    /// No ports stated: every port may reach any memory.
    pub fn none() -> Self {
        Self(|| &[])
    }
}

impl std::fmt::Debug for PortTable {
    fn fmt(
        &self,
        formatter: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        formatter.write_str("PortTable")
    }
}

impl PartialEq for PortTable {
    fn eq(
        &self,
        _: &Self,
    ) -> bool {
        true
    }
}

impl Eq for PortTable {}

impl std::hash::Hash for PortTable {
    fn hash<H: std::hash::Hasher>(
        &self,
        _: &mut H,
    ) {
    }
}

/// What a CPU's profile states that a target prices its operations from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuPrices {
    /// Clocks per kind of instruction form.
    pub costs: Vec<(String, i64)>,
    /// What an operand-size prefix costs.
    pub prefix: i64,
    /// What an address-size prefix costs besides.
    pub address_stall: i64,
    /// Registers an allocator may hold values in, and those a call keeps.
    pub registers: i64,
    pub call_registers: i64,
    /// The floating values it holds in registers at once.
    pub float_registers: i64,
    /// The indexed addresses the target states, native form first.
    pub address_forms: Vec<AddressForm>,
    /// What each operation costs: the target's mapping applied to `costs`.
    pub operations: OperationCosts,
    /// The target's address spaces by role.
    pub spaces: llrm_mir::spaces::Spaces,
    /// What its description gives a function nothing outside the program
    /// reaches.
    pub private: Option<llrm_mir::target::PrivateConvention>,
    /// The conventions as the description states them.
    pub calling: Option<&'static calling::Calling>,
    /// The platform's linear [low, high) ranges no program data occupies, for a
    /// flat target (`Machine::flat_foreign`): there is no selector to find them
    /// by.
    pub foreign: Foreign,
    /// The platform's ports and what an access to each may do to memory.
    pub ports: PortTable,
}

/// A cost model that is only what a target describes: its registers, address
/// forms and operation prices.
pub fn described(prices: &CpuPrices) -> Rc<dyn llrm_mir::target::Machine> {
    described_by_size(prices, None)
}

/// `described`, with what each operation costs in code bytes where the
/// description states them.
pub fn described_by_size(
    prices: &CpuPrices,
    sizes: Option<OperationCosts>,
) -> Rc<dyn llrm_mir::target::Machine> {
    described_with(prices, sizes, None, None)
}

/// `described_by_size`, with the target's LLVM cost model (`Machine::tti`).
pub fn described_with(
    prices: &CpuPrices,
    sizes: Option<OperationCosts>,
    tti: Option<Rc<dyn llrm_mir::tti::Tti>>,
    lowering: Option<Rc<dyn llrm_mir::lowering::Lowering>>,
) -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Described {
        tti,
        lowering,
        registers: prices.registers,
        call_registers: prices.call_registers,
        float_registers: prices.float_registers,
        address_forms: prices.address_forms.clone(),
        operations: prices.operations.clone(),
        // The by-pieces ratios are the CPU's, whichever table prices the rest.
        sizes: sizes.map(|sizes| OperationCosts {
            pieces_set: prices.operations.pieces_set,
            pieces_clear: prices.operations.pieces_clear,
            pieces_set_size: prices.operations.pieces_set_size,
            pieces_clear_size: prices.operations.pieces_clear_size,
            ..sizes
        }),
        spaces: prices.spaces,
        private: prices.private.clone(),
        calling: prices.calling,
        foreign: prices.foreign,
        ports: prices.ports,
    })
}

struct Described {
    tti: Option<Rc<dyn llrm_mir::tti::Tti>>,
    lowering: Option<Rc<dyn llrm_mir::lowering::Lowering>>,
    registers: i64,
    call_registers: i64,
    float_registers: i64,
    address_forms: Vec<AddressForm>,
    operations: OperationCosts,
    sizes: Option<OperationCosts>,
    spaces: llrm_mir::spaces::Spaces,
    private: Option<llrm_mir::target::PrivateConvention>,
    calling: Option<&'static calling::Calling>,
    foreign: Foreign,
    ports: PortTable,
}

impl llrm_mir::target::Machine for Described {
    /// The platform's table: a port it does not list may reach any memory.
    fn port_touches_memory(
        &self,
        ports: (i64, i64),
    ) -> bool {
        machine::port_reach((self.ports.0)(), ports) != machine::PortMemory::None
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
        self.spaces
    }

    /// Memory without segments is linear, the offset the address: the
    /// platform's ranges (`CpuPrices::foreign`) hold it or not.
    fn foreign_span(
        &self,
        selectors: (i64, i64),
        offsets: (i64, i64),
        width: i64,
    ) -> Option<(i64, i64)> {
        let linear = |(low, high): (i64, i64)| 0 <= low && low <= high && high <= 0xFFFF_FFFF;
        if selectors != (0, 0) || !linear(offsets) {
            return None;
        }
        machine::covered((self.foreign.0)(), offsets.0, offsets.1 + width)
    }

    fn costs(&self) -> OperationCosts {
        self.operations.clone()
    }

    fn tti(&self) -> Option<Rc<dyn llrm_mir::tti::Tti>> {
        self.tti.clone()
    }

    fn lowering(&self) -> Option<Rc<dyn llrm_mir::lowering::Lowering>> {
        self.lowering.clone()
    }

    fn size_costs(&self) -> OperationCosts {
        self.sizes.clone().unwrap_or_else(|| self.costs())
    }

    fn keeps_division(
        &self,
        bits: u32,
        size: bool,
    ) -> bool {
        keeps_division(i64::from(bits / 8), size)
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

    fn two_address(&self) -> bool {
        true
    }

    fn address_forms(&self) -> Vec<AddressForm> {
        self.address_forms.clone()
    }
}

/// Whether a division by a constant of `bytes`-byte values stays a
/// `div`/`idiv`: a word (no multiply-high), or any size when the build is for
/// size. The pin's `isIntDivCheap` is `minsize` only (DEVIATE: main keeps the
/// divide at -Os, which the size bar holds to). The one answer both the
/// instruction selector's reciprocal (`backend::division`) and the IR's
/// remainder fold read.
pub fn keeps_division(
    bytes: i64,
    size: bool,
) -> bool {
    bytes != 4 || size
}

/// A target's cost model, built from a CPU's prices: what the passes ask of it.
pub type CostModel = fn(&CpuPrices) -> Rc<dyn llrm_mir::target::Machine>;

/// A target, as the driver picks one by name: what a frontend of it starts
/// from. The passes' view of a target grows here as the backend stops naming
/// one (docs/machine/targets.md).
pub trait Target {
    /// The target's name, for messages.
    fn name(&self) -> &'static str;

    /// The platform description a frontend of this target defaults to.
    fn machine(&self) -> Machine;

    /// Whether `--isel` unnamed selects through the DAG, not the old selector.
    fn selects_by_dag(&self) -> bool {
        false
    }

    /// The linear ranges its platform states no program data occupies, where it
    /// is flat and states its selector's base as 0 (`Machine::flat_foreign`),
    /// worked out once, not for each profile.
    fn flat_foreign(&self) -> Foreign {
        Foreign::none()
    }

    /// The ports its platform states and what each may do to memory, worked out
    /// once, not for each profile.
    fn flat_ports(&self) -> PortTable {
        PortTable::none()
    }

    /// Whether the machine has no instruction for `operation` (its
    /// description's `expand`), so the compiler expands it
    /// before selection.
    fn expands(
        &self,
        operation: &str,
    ) -> bool {
        self.layout().expands(operation)
    }

    /// The processors this target prices, as a platform description names them.
    fn cpus(&self) -> &'static [&'static str];

    /// The CPU gcc's `-march=NAME` names, from the target's `timings.times`.
    fn march(
        &self,
        name: &str,
    ) -> Option<&'static str>;

    /// The names `-march` and `-mtune` take.
    fn marches(&self) -> Vec<&'static str>;

    /// The data layout programs are built under and the address spaces a
    /// frontend's addresses are.
    fn layout(&self) -> layout::Layout;

    /// The bytes an argument of `width` takes on the stack, and the least a
    /// stack cell holds.
    fn stack_slot_bytes(&self) -> i64;

    /// The bytes the stack pointer is a multiple of at a call (`calling.toml`'s
    /// `stack_alignment`): a slot's worth unless it says more.
    fn stack_alignment(&self) -> i64 {
        self.stack_slot_bytes()
    }

    /// Whether every call finds the stack a multiple of `stack_alignment`
    /// (`calling.toml`'s `aligned_calls`), not only those of a function
    /// that holds a double.
    fn aligned_calls(&self) -> bool {
        false
    }

    /// The register a frame's cells are addressed through.
    fn frame_register(&self) -> RegId;

    /// The pressure sets of the register classes and each set's limit, for a
    /// target that has them (the pre-RA scheduler tracks pressure per set).
    fn pressure(&self) -> Option<(&'static pressure::PressureSets, Vec<u32>)> {
        None
    }

    /// Where the first argument lies from the frame register: past the saved
    /// frame register and the return address, which a far call makes longer.
    fn first_argument_offset(
        &self,
        far: bool,
    ) -> i64;

    /// What a call leaves on the stack before the callee's frame: the return
    /// address, which a far call makes longer.
    fn return_address_bytes(
        &self,
        far: bool,
    ) -> i64;

    /// The register file the target's description states: what every query of
    /// a register asks, so a process can compile for several targets at once.
    fn registers(&self) -> &'static llrm_lir::registers::Info;

    /// The registers a result of `width` bytes leaves in, low part first.
    fn results(
        &self,
        width: u32,
    ) -> Vec<RegId>;

    /// The register that holds the stack's top.
    fn stack_pointer(&self) -> RegId;

    /// The registers a string operation names, where they are fixed (real
    /// mode's, whose source segment defaults to DS); none where the target
    /// has no segments to name.
    fn string_registers(&self) -> Option<StringRegisters> {
        None
    }

    /// The registers a callee keeps for its caller that an allocator may hold
    /// a value in: each by its full register and by the one a prologue pushes.
    fn callee_saved(&self) -> Vec<(RegId, RegId)>;

    /// The register `calling.toml` or a description names `name` (`ebx`,
    /// `st0`, `x19`), as the family spells it.
    fn register_named(
        &self,
        name: &str,
    ) -> RegId;

    /// The registers a result `width` bytes wide leaves in under `convention`.
    fn convention_results(
        &self,
        convention: &calling::Convention,
        width: u32,
    ) -> Vec<RegId>;

    /// What `convention` keeps for its caller: each register by its full
    /// register and the one pushed.
    fn convention_callee_saved(
        &self,
        convention: &calling::Convention,
    ) -> Vec<(RegId, RegId)>;

    /// One CPU's timings (a column of the target's `timings.times`), if the
    /// target prices it.
    fn cpu_table(
        &self,
        name: &str,
    ) -> Option<timings::CpuTable>;

    /// The target's register file, in `registers.regs`'s format.
    fn registers_text(&self) -> String;

    /// The target's instruction forms: the family's and its own, in
    /// `x86.instr`'s format.
    fn forms_text(&self) -> String;

    /// What the reader of `forms_text` must know of this target's family.
    fn dialect(&self) -> &'static instr::Dialect;

    /// The operand size an instruction has without a size prefix, in bytes.
    fn operand_bytes(&self) -> i64;

    /// The inline routine that fills `cell`-byte cells through a huge pointer.
    fn huge_fill_routine(
        &self,
        cell: u32,
    ) -> Result<routines::HugeString, String>;

    /// The inline routine that copies through huge pointers, upward or, `down`,
    /// from the last byte.
    fn huge_copy_routine(
        &self,
        down: bool,
    ) -> Result<routines::HugeString, String>;

    /// Whether the target links a C math library (`log`, `exp`, `log2` ...). It
    /// has none: the natural logarithm and the exponentials are x87
    /// sequences the finalizer spells (`fln`, `fexp`, `flog2`), not calls.
    fn has_libm(&self) -> bool {
        false
    }

    /// The inline 64-bit divide, signed or not, by a divisor of two dwords or
    /// one.
    fn divide_routine(
        &self,
        signed: bool,
        wide_divisor: bool,
    ) -> Result<routines::Divide, String>;

    /// `divide_routine` for a caller that reads only `used` of the results: a
    /// target may have a shorter routine that leaves the rest unfixed.
    fn divide_routine_used(
        &self,
        signed: bool,
        wide_divisor: bool,
        _used: routines::Used,
    ) -> Result<routines::Divide, String> {
        self.divide_routine(signed, wide_divisor)
    }

    /// The width a float value of `bytes` bytes is held at: x87 holds every one
    /// as its stack register, a vector file as its own size.
    fn float_width(
        &self,
        bytes: u32,
    ) -> u32;

    /// Bytes of a two-register operation on `width`-byte registers.
    fn register_op_bytes(
        &self,
        width: i64,
    ) -> i64;

    /// Bytes of a shift of a `width`-byte register by `count`.
    fn shift_op_bytes(
        &self,
        count: i64,
        width: i64,
    ) -> i64;

    /// Bytes of a multiply of a `width`-byte register by the immediate
    /// `number`.
    fn multiply_immediate_bytes(
        &self,
        number: i64,
        width: i64,
    ) -> i64;

    /// The CPU a compile is priced for where none is asked.
    fn default_cpu(&self) -> &'static str;

    /// `TargetLowering::getPrefFunctionAlignment` as a power of two (X86: 16
    /// bytes, 4); 0: functions are not aligned.
    fn pref_function_alignment(&self) -> u8 {
        0
    }

    /// `TargetLowering::getPrefLoopAlignment` as a power of two (X86: 16
    /// bytes, 4); 0: loops are not aligned.
    fn pref_loop_alignment(&self) -> u8 {
        0
    }

    /// `X86AsmBackend::getMaximumNopSize`: the longest single no-operation
    /// the padding of this target's code may use.
    fn max_nop_bytes(&self) -> u8 {
        1
    }

    /// The operations priced by `price` (the CPU's clocks of a form), as the
    /// target's `opcosts.txt` makes them of forms; `prefix` is the CPU's
    /// operand-size prefix cost.
    fn operation_costs(
        &self,
        price: &dyn Fn(&str) -> i64,
        prefix: i64,
    ) -> OperationCosts;

    /// The registers an allocator may hold values in.
    fn register_capacity(&self) -> i64;

    /// How deep the floating register stack is: the roots of class `x87` its
    /// register file states.
    fn float_stack(&self) -> usize {
        registers::parse(&self.registers_text()).map_or(0, |all| registers::of_class(&all, "x87").len())
    }

    /// Whether scalar floating point lives in SSE registers: its register
    /// file states roots of class `xmm`.
    fn scalar_sse(&self) -> bool {
        registers::parse(&self.registers_text()).is_ok_and(|all| !registers::of_class(&all, "xmm").is_empty())
    }

    /// The indexed addresses a memory access may use, priced by `costs` (its
    /// operand-size prefix in `prefix`) and `address_stall`: native form first.
    fn address_forms(
        &self,
        costs: &OperationCosts,
        address_stall: i64,
    ) -> Vec<AddressForm>;

    /// How this target's passes are given the prices of a CPU.
    fn cost_model(&self) -> CostModel;

    /// The calling conventions a program for this target may name, by the names
    /// `calling.toml` gives them; the first is the one a language's own
    /// functions use.
    fn conventions(&self) -> &'static [&'static str];

    /// The calling conventions themselves, `calling.toml` read: where each
    /// passes arguments, what it keeps.
    fn calling(&self) -> &'static calling::Calling;

    /// The machine's physical addresses by name (the text screen's video
    /// memory): one fact, which a target's address space turns into the
    /// pointer it has.
    fn physical_addresses(&self) -> Vec<(String, u64)> {
        Vec::new()
    }

    /// The OS layer a language's runtime is built on here, if the target has
    /// one for it.
    fn runtime(
        &self,
        _language: &str,
    ) -> Option<runtime::Description> {
        None
    }

    /// The OS layer every language's runtime on this target calls: the one
    /// interface, implemented here.
    fn os_layer(&self) -> Option<os::Layer> {
        None
    }

    /// The object file and listing this target writes.
    fn object(&self) -> object::ObjectFormat;

    /// What a frame is built of.
    fn frame_registers(&self) -> FrameRegisters {
        FrameRegisters {
            pointer: self.frame_register(),
            stack: self.stack_pointer(),
            saved: self.callee_saved(),
            slot: self.stack_slot_bytes(),
            align: self.stack_alignment(),
            optional: self.frame_optional(),
            enter: self.frame_enter(),
            wrap_split_size: self.frame_wrap_split_size(),
            slot_scratch: self.frame_slot_scratch(),
            free: false,
        }
    }

    /// Whether a callee may take its arguments off the stack as it returns
    /// (some convention of `calling.toml` has `cleanup = "callee"`): where none
    /// does, a call through a pointer leaves the stack where it found it.
    fn callee_pops(&self) -> bool {
        true
    }

    /// Whether a signed divide whose quotient overflows (`INT_MIN / -1`)
    /// raises the processor's divide fault, which the runtime installs a
    /// handler for and reports as BASIC's error 11: a frontend may then leave
    /// out its own check of that case. Off where no handler is known.
    fn idiv_overflow_traps(&self) -> bool {
        false
    }

    /// Whether a function that needs no frame register may leave it out
    /// (`calling.toml`'s `frame_optional`).
    fn frame_optional(&self) -> bool {
        false
    }

    /// Whether a frame tuned for size opens with `enter` (`calling.toml`'s
    /// `frame_enter`).
    fn frame_enter(&self) -> bool {
        false
    }

    /// Whether a size build wraps a frame around a shared return
    /// (`calling.toml`'s `frame_wrap_split_size`).
    /// The registers a one-slot frame pushes and pops, where the target has
    /// them.
    fn frame_slot_scratch(&self) -> Option<(RegId, RegId)> {
        None
    }

    fn frame_wrap_split_size(&self) -> bool {
        false
    }
}

/// The registers a procedure's frame is made of. LIR names the frame register
/// BP and the stack pointer SP whatever the target; a listing and an object
/// spell them as the target has them.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FrameRegisters {
    pub pointer: RegId,
    pub stack: RegId,
    /// Each callee-saved register by its full register and the one pushed.
    pub saved: Vec<(RegId, RegId)>,
    /// The bytes the stack is kept a multiple of.
    pub slot: i64,
    /// The bytes the stack pointer is a multiple of at a call, as the callee
    /// assumes it (its frame cells then sit on their alignments): a slot's
    /// worth where the target says no more.
    pub align: i64,
    /// A function that needs no frame register may leave it out.
    pub optional: bool,
    /// A frame tuned for size opens with `enter`.
    pub enter: bool,
    /// A size build may skip the frame on an early exit into a shared return.
    pub wrap_split_size: bool,
    /// The registers a one-slot frame pushes and pops (`calling.toml`'s
    /// `frame_slot_push`/`frame_slot_pop`), where it does.
    pub slot_scratch: Option<(RegId, RegId)>,
    /// The function has no frame register: it holds a value, and a frame cell
    /// is addressed through the stack pointer.
    pub free: bool,
}

impl FrameRegisters {
    /// `register` as this target spells it: the frame register and stack
    /// pointer LIR calls BP and SP.
    pub fn spelled(
        &self,
        register: RegId,
    ) -> RegId {
        match register {
            // Where the frame register holds a value, BP is its word view, not
            // the frame token.
            RegId::BP if !self.free => self.pointer,
            RegId::SP => self.stack,
            other => other,
        }
    }
}

/// The I/O ports of a PC: a `[[ports]]`-only description that a platform
/// appends to its own text.
pub const PC_PORTS: &str = include_str!("../../../../machine/platform/dos/ports.toml");
