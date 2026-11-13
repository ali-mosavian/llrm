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
pub mod schedule;
pub mod timings;
pub mod tune;
pub mod testing;

use std::rc::Rc;

use llrm_lir::registers::RegId;
use llrm_mir::target::{AddressForm, OperationCosts};
use machine::Machine;

/// The encoded sizes a target states of a register copy and a constant load.
#[derive(Clone, Copy, Debug)]
pub struct MoveBytes {
    /// The widest operand an instruction takes without a size prefix (a REX.W
    /// qword at m64 has one), in bytes; the `operand` the functions below take.
    pub plain: i64,
    /// Bytes of a copy between two `width`-byte registers.
    pub copy: fn(width: i64, operand: i64) -> i64,
    /// Bytes of loading `number` into a `width`-byte register; `address`: it
    /// is a symbol's address, whose bytes the immediate takes whole.
    pub literal: fn(number: i64, address: bool, width: i64, operand: i64) -> i64,
}

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

/// The registers a string instruction (`rep movs`, `rep stos`) uses, each by its
/// root: the view of the pointer's width is the operand (`cx`, `ecx`, `rcx`).
#[derive(Clone, Copy, Debug)]
pub struct StringRoots {
    pub count: RegId,
    pub source: RegId,
    pub destination: RegId,
    /// The data register a store or compare is made through (`al`, `ax`, `eax`, `rax`).
    pub accumulator: RegId,
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
    described_throughput(prices, sizes, tti, lowering, None)
}

/// `described_with`, and the target's throughput model (`Machine::throughput`).
pub fn described_throughput(
    prices: &CpuPrices,
    sizes: Option<OperationCosts>,
    tti: Option<Rc<dyn llrm_mir::tti::Tti>>,
    lowering: Option<Rc<dyn llrm_mir::lowering::Lowering>>,
    throughput: Option<llrm_mir::target::Throughput>,
) -> Rc<dyn llrm_mir::target::Machine> {
    Rc::new(Described {
        throughput,
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
    throughput: Option<llrm_mir::target::Throughput>,
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
    fn throughput(&self) -> Option<&llrm_mir::target::Throughput> {
        self.throughput.as_ref()
    }

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

/// What the DAG builder asks of a target for every target. The x86-style paths
/// (calling.toml conventions, string instructions, libcall routines) read the
/// whole [`Target`] through [`BuilderArch::full`], which a target without one
/// answers with `None` and the builder refuses.
pub trait BuilderArch {
    fn registers(&self) -> &'static llrm_lir::registers::Info;
    fn register_named(
        &self,
        name: &str,
    ) -> RegId;
    fn stack_slot_bytes(&self) -> i64;
    fn stack_alignment(&self) -> i64;
    /// The address-space roles (`layout().spaces.roles`).
    fn roles(&self) -> llrm_mir::spaces::Spaces;
    fn full(&self) -> Option<&dyn Target>;
    /// The stack pointer at the pointer width of the target's code: the
    /// register file's stack root unless the target says otherwise.
    fn stack_pointer(&self) -> RegId {
        self.registers().stack
    }
    /// The registers of the target's string instructions, if it has them.
    fn string_roots(&self) -> Option<StringRoots> {
        None
    }
    /// The DAG's name for the top of the x87 stack, if the target has one.
    fn x87_stack_top(&self) -> Option<RegId> {
        None
    }
}

impl<T: Target> BuilderArch for T {
    fn registers(&self) -> &'static llrm_lir::registers::Info {
        Target::registers(self)
    }
    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        Target::register_named(self, name)
    }
    fn stack_slot_bytes(&self) -> i64 {
        Target::stack_slot_bytes(self)
    }
    fn stack_alignment(&self) -> i64 {
        Target::stack_alignment(self)
    }
    fn roles(&self) -> llrm_mir::spaces::Spaces {
        Target::address_roles(self)
    }
    fn full(&self) -> Option<&dyn Target> {
        Some(self)
    }
    fn stack_pointer(&self) -> RegId {
        Target::stack_pointer(self)
    }
    fn string_roots(&self) -> Option<StringRoots> {
        Target::string_roots(self)
    }
    fn x87_stack_top(&self) -> Option<RegId> {
        Target::x87_stack_top(self)
    }
}

impl BuilderArch for &dyn Target {
    fn registers(&self) -> &'static llrm_lir::registers::Info {
        Target::registers(*self)
    }
    fn register_named(
        &self,
        name: &str,
    ) -> RegId {
        Target::register_named(*self, name)
    }
    fn stack_slot_bytes(&self) -> i64 {
        Target::stack_slot_bytes(*self)
    }
    fn stack_alignment(&self) -> i64 {
        Target::stack_alignment(*self)
    }
    fn roles(&self) -> llrm_mir::spaces::Spaces {
        Target::address_roles(*self)
    }
    fn stack_pointer(&self) -> RegId {
        Target::stack_pointer(*self)
    }
    fn string_roots(&self) -> Option<StringRoots> {
        Target::string_roots(*self)
    }
    fn x87_stack_top(&self) -> Option<RegId> {
        Target::x87_stack_top(*self)
    }
    fn full(&self) -> Option<&dyn Target> {
        Some(*self)
    }
}

/// Which register allocator compiles a target's functions
/// (`TargetPassConfig::createRegAllocPass`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Allocator {
    /// The translated greedy allocator, which the pin runs for every target
    /// that has one.
    Greedy,
    /// The spike graph-colouring allocator (`llrm-spike-mf`): a target waits
    /// on it until its greedy port lands.
    Spike,
    /// An allocator of the target's own.
    Own,
    /// None: the pin's target replaces register allocation (wasm's
    /// `addRegAssignAndRewriteOptimized` returns false).
    None,
}

/// A target, as the driver picks one by name: what a frontend of it starts
/// from. The passes' view of a target grows here as the backend stops naming
/// one (docs/machine/targets.md).
/// x86's `isAsCheapAsAMove` of a constant: `MOV64ri` (`movabs`), an immediate
/// that neither a sign-extended nor a zero-extended dword makes, is
/// rematerializable but not cheap (X86InstrMisc.td, outside the `MOV8ri`..
/// `MOV64ri32` block).
pub fn x86_cheap_move_immediate(
    bytes: u8,
    value: i64,
) -> bool {
    !(bytes == 8 && !(i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&value))
}

/// A constant a build for size selects as a pseudo that is rematerializable
/// but not `isAsCheapAsAMove`: outside 64-bit mode x86 makes +1 and -1 with
/// `xor` and `inc`/`dec` (`MOV32r1`, `MOV32r_1`, X86InstrCompiler.td, under
/// `OptForSize, Not64BitMode`), so `RegisterCoalescer::reMaterializeDef`
/// leaves a copy of one where it is.
pub fn x86_uncheap_move_immediate_at_size(
    bytes: u8,
    value: i64,
) -> bool {
    matches!(bytes, 2 | 4) && matches!(value, 1 | -1)
}

pub trait Target {
    /// The target's name, for messages.
    fn name(&self) -> &'static str;

    /// Its object writers and what each format says of the code it holds
    /// (LLVM's `MCObjectWriter` and `MCAsmInfo`), which the backend is handed
    /// instead of naming a target.
    fn objects(&self) -> &'static dyn llrm_debug::DebugObjectTarget;

    /// The machine as a debug writer reads it (`MCRegisterInfo` and the
    /// fixup kinds of a segment): everything but the register file, which the
    /// caller reads from [`Target::registers_text`] once.
    fn debug_target(&self) -> llrm_debug::DebugTarget;

    /// The platform description a frontend of this target defaults to.
    fn machine(&self) -> Machine;

    /// The allocator that compiles this target's functions: the pin's choice
    /// unless the target says otherwise.
    fn register_allocator(&self) -> Allocator {
        Allocator::Greedy
    }

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

    /// The processors this target prices, as a platform description names them;
    /// none: any `-mcpu` is refused.
    fn cpus(&self) -> &'static [&'static str] {
        &[]
    }

    /// The CPU gcc's `-march=NAME` names, from the target's `timings.times`.
    fn march(
        &self,
        _name: &str,
    ) -> Option<&'static str> {
        None
    }

    /// The names `-march` and `-mtune` take; none where it has no table.
    fn marches(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// The data layout programs are built under and the address spaces a
    /// frontend's addresses are.
    fn layout(&self) -> layout::Layout;

    /// `layout().spaces.roles`, without cloning the layout: asked per function.
    fn address_roles(&self) -> llrm_mir::spaces::Spaces {
        self.layout().spaces.roles
    }

    /// The bytes an argument of `width` takes on the stack, and the least a
    /// stack cell holds.
    fn stack_slot_bytes(&self) -> i64;

    /// The bytes the stack pointer is a multiple of at a call (`calling.toml`'s
    /// `stack_alignment`): a slot's worth unless it says more.
    fn stack_alignment(&self) -> i64 {
        self.stack_slot_bytes()
    }

    /// Whether a frame can realign the stack pointer for a local above the
    /// stack alignment (the pin's `X86RegisterInfo::canRealignStack`,
    /// [X86RegisterInfo.cpp#L823](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/X86/X86RegisterInfo.cpp#L823)).
    /// Where it cannot, no local is over-aligned: the pin clamps the object's
    /// alignment to the stack alignment
    /// (`MachineFrameInfo::clampStackAlignment`, MachineFrameInfo.cpp#L40);
    /// here the cell keeps its alignment and the stack is left as the
    /// caller made it (DEVIATE OA-2, owner w2, exit: m16 states its own
    /// stack alignment).
    fn can_realign_stack(&self) -> bool {
        true
    }

    /// Whether every call finds the stack a multiple of `stack_alignment`
    /// (`calling.toml`'s `aligned_calls`), not only those of a function
    /// that holds a double.
    fn aligned_calls(&self) -> bool {
        false
    }

    /// The machine phases (`flow::machine_with`'s class names) this target does
    /// not run, each with why. The pin's `TargetPassConfig` adds each target's
    /// own passes while the phase list here is x86's, so a target names what
    /// its pin has no pass for. An unknown name is an error in `machine_with`.
    fn omitted_phases(&self) -> &'static [(&'static str, &'static str)] {
        &[]
    }

    /// The register a frame's cells are addressed through.
    fn frame_register(&self) -> RegId;

    /// A `mov imm; jmp` block with one predecessor is folded into it at -Os
    /// (LEAF-FOLD-OS).
    fn leaf_fold_at_size(&self) -> bool {
        false
    }

    /// MachineCSE keeps a compare whose difference nothing reads apart from the
    /// same difference made in a later block, when that difference overwrites
    /// an operand still live (CSE-CMP): sharing them moves the subtract in
    /// front of the branch and adds a copy of the operand. The pin shares.
    fn cse_keeps_compare_with_live_minuend(&self) -> bool {
        false
    }

    /// `TargetInstrInfo::isAsCheapAsAMove` for a constant: whether a
    /// `bytes`-wide register can be given `value` by one move that is as
    /// cheap as a copy (MachineSink, MachineCSE and MachineLICM leave it
    /// where it is). The default is x86's: every immediate but a `movabs`.
    /// AArch64 asks whether the constant takes two instructions or fewer
    /// (`AArch64InstrInfo.cpp#L1385`).
    fn cheap_move_immediate(&self) -> fn(u8, i64) -> bool {
        x86_cheap_move_immediate
    }

    /// A constant that, in a build for size, is not `isAsCheapAsAMove` though
    /// `cheap_move_immediate` says it is (see
    /// [`x86_uncheap_move_immediate_at_size`]). The default has none.
    fn uncheap_move_immediate_at_size(&self) -> fn(u8, i64) -> bool {
        |_, _| false
    }

    /// Whether `MachineLICM` refuses a hoist by the register pressure of the
    /// loop (the pin's `CanCauseHighRegPressure`); false: by the count of
    /// values the loop names against the registers (the rule before it).
    fn hoist_by_pressure(&self) -> bool {
        true
    }

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

    /// The registers of this target's string instructions, where it has them.
    fn string_roots(&self) -> Option<StringRoots> {
        None
    }

    /// The register the DAG names the top of the x87 stack by (the pin's
    /// `FP0`), where the target has an x87 unit.
    fn x87_stack_top(&self) -> Option<RegId> {
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
        _name: &str,
    ) -> Option<timings::CpuTable> {
        None
    }

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
        _cell: u32,
    ) -> Result<routines::HugeString, String> {
        Err(format!("{}: huge_fill_routine: the target has no huge pointers", self.name()))
    }

    /// The inline routine that copies through huge pointers, upward or, `down`,
    /// from the last byte.
    fn huge_copy_routine(
        &self,
        _down: bool,
    ) -> Result<routines::HugeString, String> {
        Err(format!("{}: huge_copy_routine: the target has no huge pointers", self.name()))
    }

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
        _signed: bool,
        _wide_divisor: bool,
    ) -> Result<routines::Divide, String> {
        Err(format!("{}: divide_routine: the target states no inline divide", self.name()))
    }

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

    /// `divide_routine_used` for code built for size: a target may have a
    /// routine shorter than the speed one; it refuses where it has none.
    fn divide_routine_small(
        &self,
        _signed: bool,
        _wide_divisor: bool,
        _used: routines::Used,
    ) -> Result<routines::Divide, String> {
        Err("no size divide routine".into())
    }

    /// Whether -O1 code (bytes and clocks both strict) takes the size divide
    /// too: only where its clocks stay under the bar; -Os always does.
    fn small_divide_at_o1(&self) -> bool {
        false
    }

    /// DEVIATE DIV-HOT: where a size build takes the size divide, an `i64` divide in a loop
    /// takes the speed routine. The size routine is a bit a round, some 600 instructions
    /// slower; the module pays the speed one's extra bytes once.
    fn fast_divide_in_loops_at_size(&self) -> bool {
        false
    }

    /// The width a float value of `bytes` bytes is held at: x87 holds every one
    /// as its stack register, a vector file as its own size.
    fn float_width(
        &self,
        bytes: u32,
    ) -> u32 {
        bytes
    }

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

    /// The encoded sizes of a register copy and a constant load, where the
    /// target states them; none: the backend asks no price of them.
    fn move_bytes(&self) -> Option<MoveBytes> {
        None
    }

    /// The CPU a compile is priced for where none is asked.
    fn default_cpu(&self) -> &'static str;

    /// Whether a `bsf` whose flags are dead carries the `rep` prefix, the
    /// TZCNT encoding (`X86MCInstLower`). A processor without `tzcnt` runs it
    /// as `bsf`, so the prefix is a dead byte there; the pin emits it for
    /// compatibility with later processors.
    fn bsf_rep_prefix(&self) -> bool {
        true
    }

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

    /// Whether loops are aligned at -O1. The pin's X86 aligns at every level
    /// above -O0; gcc's `-falign-loops` starts at -O2.
    fn align_loops_at_o1(&self) -> bool {
        true
    }

    /// The most bytes of padding a loop header's alignment may take (the
    /// `MaxBytesForAlignment` the pin's X86 never sets, gcc's `.p2align
    /// 4,,10`); 0: no limit.
    fn loop_alignment_max_skip(&self) -> u8 {
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
        registers::float_facts(&self.registers_text()).0
    }

    /// Whether scalar floating point lives in SSE registers: its register
    /// file states roots of class `xmm`.
    fn scalar_sse(&self) -> bool {
        registers::float_facts(&self.registers_text()).1
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
            vector_saved: Vec::new(),
            slot: self.stack_slot_bytes(),
            align: self.stack_alignment(),
            object_align: self.stack_alignment(),
            optional: self.frame_optional(),
            saved_first: self.frame_saved_first(),
            enter: self.frame_enter(),
            wrap_split_size: self.frame_wrap_split_size(),
            object_aligned: self.frame_object_aligned(),
            red_zone: self.frame_red_zone(),
            slot_scratch: self.frame_slot_scratch(),
            free: false,
            kept: false,
            exit_cfi: true,
        }
    }

    /// Whether a callee may take its arguments off the stack as it returns
    /// (some convention of `calling.toml` has `cleanup = "callee"`): where none
    /// does, a call through a pointer leaves the stack where it found it.
    fn callee_pops(&self) -> bool {
        true
    }

    /// Whether a function that needs no frame register may leave it out
    /// (`calling.toml`'s `frame_optional`).
    fn frame_optional(&self) -> bool {
        false
    }

    /// Whether the registers a function keeps are pushed before the space for
    /// its locals even where it calls nothing (`calling.toml`'s
    /// `frame_saved_first`).
    fn frame_saved_first(&self) -> bool {
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

    /// The registers a one-slot frame's `pop` may take, first choice first
    /// (`X86RegisterInfo::findDeadCallerSavedReg` walks `GR32_NOSP`): the first
    /// that no return takes a result in and the function does not save. Empty
    /// where the target keeps the pair its convention declares.
    fn frame_slot_pop_order(&self) -> Vec<RegId> {
        Vec::new()
    }

    /// `frame_slot_scratch` for a function of `convention`, where the target's
    /// frames differ by convention (m32's DOS ones keep Watcom's layout).
    fn frame_slot_scratch_of(
        &self,
        _convention: &crate::calling::Convention,
    ) -> Option<(RegId, RegId)> {
        self.frame_slot_scratch()
    }

    fn frame_wrap_split_size(&self) -> bool {
        false
    }

    /// Whether the frame aligns its locals to the widest object
    /// (`calling.toml`'s `frame_object_aligned`).
    fn frame_object_aligned(&self) -> bool {
        false
    }

    /// The bytes below the stack pointer a leaf frame may use (`calling.toml`'s
    /// `frame_red_zone`).
    fn frame_red_zone(&self) -> i64 {
        0
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
    /// The callee-saved vector registers (Win64's xmm6-15), each by its full
    /// register and itself: saved with `movaps` into the frame, not pushed.
    pub vector_saved: Vec<(RegId, RegId)>,
    /// The bytes the stack is kept a multiple of.
    pub slot: i64,
    /// The bytes the stack pointer is a multiple of at a call, as the callee
    /// assumes it (its frame cells then sit on their alignments): a slot's
    /// worth where the target says no more.
    pub align: i64,
    /// The most an object of the frame is aligned to: `align`, but for a
    /// convention that keeps the stack wider than the target's objects need.
    pub object_align: i64,
    /// A function that needs no frame register may leave it out.
    pub optional: bool,
    /// The registers are pushed before the locals' space even where nothing is
    /// called.
    pub saved_first: bool,
    /// A frame tuned for size opens with `enter`.
    pub enter: bool,
    /// A size build may skip the frame on an early exit into a shared return.
    pub wrap_split_size: bool,
    /// The locals' base takes the widest object's alignment (`frame_object_aligned`).
    pub object_aligned: bool,
    /// The bytes below the stack pointer a leaf function's locals may use
    /// without a `sub`; 0 where the ABI has no red zone.
    pub red_zone: i64,
    /// The registers a one-slot frame pushes and pops (`calling.toml`'s
    /// `frame_slot_push`/`frame_slot_pop`), where it does.
    pub slot_scratch: Option<(RegId, RegId)>,
    /// The function has no frame register: it holds a value, and a frame cell
    /// is addressed through the stack pointer.
    pub free: bool,
    /// The function's `"frame-pointer"` attribute keeps the frame register
    /// even where nothing needs it
    /// (`MachineFunction::disableFramePointerElim`).
    pub kept: bool,
    /// The exits state the frame address as they move it
    /// (`X86FrameLowering::emitEpilogue`'s `NeedsDwarfCFI`); an object
    /// format whose unwinder does not read them (Mach-O, COFF) turns it off.
    pub exit_cfi: bool,
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
