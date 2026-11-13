//! Register allocation for the m68k machine function: m68k's register
//! classes behind the neutral allocator of [`llrm_spike_mf`].
//!
//! The classes are `DR8`/`DR16`/`DR32` (the views of `d0` to `d7`), `AR32`
//! (`a0` to `a5`; `a6` and `sp` are the frame and stack) and `XR32` (both).

use llrm_lir::registers::RegId;
use llrm_spike_mf::regalloc::Target;
pub use llrm_spike_mf::regalloc::{Allocate, Assignment};
use llrm_spike_mf::{Jump, Kind, Operand, Reg};

use crate::mfunc::{Class, MFunction};
use crate::{REGISTER_INFO, reg};

const DATA: [RegId; 8] = [reg::D0, reg::D1, reg::D2, reg::D3, reg::D4, reg::D5, reg::D6, reg::D7];
const ADDRESS: [RegId; 6] = [reg::A0, reg::A1, reg::A2, reg::A3, reg::A4, reg::A5];

/// The registers a call leaves unspecified (`CSR_STD` keeps the rest), data
/// registers first as the pin has them.
const CALLER_SAVED: [RegId; 4] = [reg::D0, reg::D1, reg::A0, reg::A1];

/// The same, address registers first (see [`Registers`]).
const CALLER_SAVED_ADDRESS_FIRST: [RegId; 4] = [reg::A1, reg::A0, reg::D0, reg::D1];

/// m68k's register file for the allocator.
///
/// DEVIATE M68K-FLEXORDER: with `ADDRESS_FIRST` a value either kind of
/// register can hold takes `a1`, `a0` before `d0`, `d1`. The pin tries the data
/// registers first, so a value only copied and compared sits in one and the
/// values that need a data register (a counter, a compare's left side) are
/// pushed to a saved one (arrmax -Os: `d2`, a push and a pop, 34 B against
/// gcc's 26). Used with [`crate::compile::Options::indexed`]; the pin's order
/// stays for the pinned output. Owner m68k. Exit: 2026-11-28, when the shared
/// greedy prices a class by what its users need.
/// Tried 2026-11-07 for fib/countgt/copy/crc16 (greedy-subreg), both reverted:
/// no narrowing join of an address-capable value (0 better, 7 of 12 kernels
/// worse) and a tied source kept in its own class (0 better, arrsum +8 B).
/// The gap needs pressure (t and i hold d0, d1; b goes to an address register),
/// which neither a join rule nor a class sees.
///
/// DEVIATE M68K-ARGREMAT (with `ARGUMENT_REMAT`): a load of an incoming stack
/// argument that nothing writes is made again where it is read, not stored to
/// a spill slot and reloaded. The pin's M68k marks no load rematerializable
/// (`isReMaterializable` needs the descriptor flag), so it spills. Reason:
/// the slot is an immutable fixed object, gcc reloads it, and a spill store
/// plus a frame cell is more bytes and clocks (seeds 300: about 115 smaller,
/// 11 larger per CPU and level). Owner m68k. Exit: 2026-11-28, or when the
/// pin's M68k marks the rows. Off for `Options::PINNED`.
pub struct Registers<
    const ADDRESS_FIRST: bool,
    const ARGUMENT_REMAT: bool,
    const GLOBAL_COPIES: bool,
    const LOOP_CARRIED: bool,
>;

/// The allocator's registers in the order this compiler uses by default.
pub type M68kRegisters = Registers<true, true, true, true>;

impl<const ADDRESS_FIRST: bool, const ARGUMENT_REMAT: bool, const GLOBAL_COPIES: bool, const LOOP_CARRIED: bool> Target
    for Registers<ADDRESS_FIRST, ARGUMENT_REMAT, GLOBAL_COPIES, LOOP_CARRIED>
{
    fn spills_named_by_lane() -> bool {
        true
    }

    type Class = Class;

    fn schedules_before_allocation() -> bool {
        llrm_support::env_flag!("LLRM_M68K_PRESCHED")
    }

    fn meet(
        one: Class,
        other: Class,
    ) -> Option<Class> {
        one.meet(other)
    }

    fn any() -> Class {
        Class::Xr32
    }

    /// An incoming argument is an immutable fixed stack object, so the pin
    /// makes a load of one again where it is read
    /// (`isLoadFromInvariantOrConstantMemory`); `MOV32rp` takes either kind of
    /// register, so the copy can land wherever its reader needs.
    fn is_invariant_argument_load(name: &str) -> bool {
        ARGUMENT_REMAT && name == "MOV32rp"
    }

    /// A made-again argument load is a `move` that sets the codes in hardware;
    /// the reader after the original reads the original's ([`crate::flags`]).
    fn remade_sets_no_flags() -> bool {
        true
    }

    /// DEVIATE C13 (coalescer/mod.rs): the joins raised `movem` 71 -> 89 in
    /// div_constants_short and made 15 of 90 cell-CPU rows worse.
    fn joins_sub_registers() -> bool {
        false
    }

    /// `JoinGlobalCopies`: true is what the allocator was tuned with; false is
    /// the pin's M68k. A function is built both ways and the cheaper kept
    /// (`compile::items_with`): false fixed `fib` -Os (the local copy joined
    /// before the global one) and made other cells worse, so neither value is
    /// right for every function.
    fn joins_global_copies() -> bool {
        GLOBAL_COPIES
    }

    /// DEVIATE C15 (coalescer/pair.rs; `Options::loop_carried_first`, off for
    /// the pin): `m = max(m, p[i])` kept a copy on both paths (arrmax -O2
    /// 68000 +160 rate against gcc; the pin has the same order and its llc
    /// is 80 B). Owner: m68k. Exit: 2026-11-22.
    fn joins_loop_carried_first() -> bool {
        LOOP_CARRIED
    }

    fn candidates(class: Class) -> Vec<RegId> {
        match class {
            Class::Dr8 | Class::Dr16 | Class::Dr32 => DATA.to_vec(),
            Class::Ar32 => ADDRESS.to_vec(),
            // the registers a call clobbers anyway first, then the saved ones:
            // a saved register costs a push and a pop (`RegisterClassInfo`
            // orders the callee-saved last)
            Class::Xr32 => (if ADDRESS_FIRST { CALLER_SAVED_ADDRESS_FIRST } else { CALLER_SAVED })
                .iter()
                .chain(DATA.iter().filter(|one| !CALLER_SAVED.contains(one)))
                .chain(ADDRESS.iter().filter(|one| !CALLER_SAVED.contains(one)))
                .copied()
                .collect(),
        }
    }

    fn operand_class(
        name: &str,
        index: usize,
    ) -> Option<Class> {
        crate::mfunc::operand_class(name, index)
    }

    /// `bd0` for a byte class, `wd0` for a word.
    fn view(
        root: RegId,
        class: Class,
    ) -> RegId {
        let bits = match class {
            Class::Dr8 => 8,
            Class::Dr16 => 16,
            _ => 32,
        };
        REGISTER_INFO.view(root, bits).unwrap_or(root)
    }

    fn root(register: RegId) -> RegId {
        REGISTER_INFO.root(register)
    }

    fn branch_to(block: u32) -> Option<llrm_spike_mf::Inst> {
        Some(<Self as llrm_spike_mf::sink::Jump>::jump(block))
    }

    fn reserved(root: RegId) -> bool {
        root == reg::SP || root == reg::A6
    }

    fn flags_register() -> Option<RegId> {
        Some(reg::CCR)
    }

    fn hoist_window() -> usize {
        4
    }

    fn caller_saved() -> Vec<RegId> {
        CALLER_SAVED.to_vec()
    }

    fn greedy_early_clobber() -> bool {
        true
    }

    /// `copyPhysReg`: a copy into a data register is a `move`, which sets the
    /// codes.
    fn copy_sets_flags(
        to: RegId,
        from: RegId,
    ) -> bool {
        to != from && crate::flags::is_data(to)
    }

    /// `getStoreRegOpcode`: a `MOVEM` pseudo, since a `move` would overwrite
    /// the condition codes a branch may still read.
    fn spill_store(
        _class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<llrm_spike_mf::Inst> {
        Some(llrm_spike_mf::Inst {
            may_store: true,
            ..llrm_spike_mf::Inst::new(
                Kind::Op("MOVM32pm_P"),
                vec![Operand::Imm(0), Operand::Frame(slot), Operand::Use(register)],
            )
        })
    }

    /// `getLoadRegOpcode`.
    fn spill_load(
        _class: Class,
        register: Reg,
        slot: i32,
    ) -> Option<llrm_spike_mf::Inst> {
        Some(llrm_spike_mf::Inst {
            may_load: true,
            ..llrm_spike_mf::Inst::new(
                Kind::Op("MOVM32mp_P"),
                vec![Operand::Def(register), Operand::Imm(0), Operand::Frame(slot)],
            )
        })
    }
}

pub type GraphColoring = llrm_spike_mf::GraphColoring<M68kRegisters>;

pub use llrm_spike_mf::regalloc::{eliminate_phis, eliminate_phis_with, successors, two_address};

pub fn fold_reserved_copies(function: &mut MFunction) {
    llrm_spike_mf::regalloc::fold_reserved_copies::<M68kRegisters>(function)
}

/// Allocation that spills what does not fit; the slots it made, with their
/// classes.
pub fn allocate_spilling(
    function: &mut MFunction,
    first_slot: i32,
) -> Result<(Assignment, Vec<(i32, Class)>), String> {
    llrm_spike_mf::spill::allocate_spilling::<M68kRegisters>(function, first_slot)
}

/// [`allocate_spilling`], with the copies a loop runs on every trip joined
/// first. The pin does the same: `RegisterCoalescer` visits blocks by loop
/// depth and `CalcSpillWeights` weighs uses by `MachineBlockFrequencyInfo`
/// (pin d1106deb71cc).
pub fn allocate_spilling_by_frequency(
    function: &mut MFunction,
    first_slot: i32,
) -> Result<(Assignment, Vec<(i32, Class)>), String> {
    llrm_spike_mf::spill::allocate_spilling_ordered::<M68kRegisters>(function, first_slot, true)
}

/// [`allocate_spilling_by_frequency`] by the translated greedy allocator.
pub fn allocate_greedy(
    function: &mut MFunction,
    size: bool,
    address_first: bool,
    rematerialize_arguments: bool,
    global_copies: bool,
    loop_carried_first: bool,
) -> Result<(Assignment, Vec<(i32, Class)>), String> {
    allocate_greedy_after(
        function,
        size,
        address_first,
        rematerialize_arguments,
        global_copies,
        loop_carried_first,
        None,
    )
}

/// [`allocate_greedy`], the function still in SSA form and `prelude` the shared
/// `PhiElimination` and `TwoAddress` ([`shared_prelude`]).
pub fn allocate_greedy_after(
    function: &mut MFunction,
    size: bool,
    address_first: bool,
    rematerialize_arguments: bool,
    global_copies: bool,
    loop_carried_first: bool,
    prelude: Option<llrm_spike_mf::tolir::Prelude>,
) -> Result<(Assignment, Vec<(i32, Class)>), String> {
    // each option is a type parameter: the allocator reads them as constants
    fn run<const A: bool, const R: bool, const G: bool, const L: bool>(
        function: &mut MFunction,
        regs: llrm_lir::registers::Regs,
        size: bool,
        prelude: Option<llrm_spike_mf::tolir::Prelude>,
    ) -> Result<(Assignment, Vec<(i32, Class)>), String> {
        llrm_spike_mf::tolir::allocate_greedy_after::<Registers<A, R, G, L>>(function, regs, size, prelude)
    }
    let regs = llrm_lir::registers::Regs(&REGISTER_INFO);
    match (address_first, rematerialize_arguments, global_copies, loop_carried_first) {
        (true, true, true, true) => run::<true, true, true, true>(function, regs, size, prelude),
        (true, true, true, false) => run::<true, true, true, false>(function, regs, size, prelude),
        (true, true, false, true) => run::<true, true, false, true>(function, regs, size, prelude),
        (true, true, false, false) => run::<true, true, false, false>(function, regs, size, prelude),
        (true, false, true, true) => run::<true, false, true, true>(function, regs, size, prelude),
        (true, false, true, false) => run::<true, false, true, false>(function, regs, size, prelude),
        (true, false, false, true) => run::<true, false, false, true>(function, regs, size, prelude),
        (true, false, false, false) => run::<true, false, false, false>(function, regs, size, prelude),
        (false, true, true, true) => run::<false, true, true, true>(function, regs, size, prelude),
        (false, true, true, false) => run::<false, true, true, false>(function, regs, size, prelude),
        (false, true, false, true) => run::<false, true, false, true>(function, regs, size, prelude),
        (false, true, false, false) => run::<false, true, false, false>(function, regs, size, prelude),
        (false, false, true, true) => run::<false, false, true, true>(function, regs, size, prelude),
        (false, false, true, false) => run::<false, false, true, false>(function, regs, size, prelude),
        (false, false, false, true) => run::<false, false, false, true>(function, regs, size, prelude),
        (false, false, false, false) => run::<false, false, false, false>(function, regs, size, prelude),
    }
}

/// `MachineSinking` over the m68k function.
pub fn machine_sink(function: &mut MFunction) -> bool {
    llrm_spike_mf::machine_sink::<M68kRegisters>(function)
}

/// `reverseBranchCondition` for `Bcc8`: the opposite condition is the
/// condition code xor 1.
const CONDITIONAL: [(&str, &str); 7] = [
    ("Bhi8", "Bls8"),
    ("Bcc8", "Bcs8"),
    ("Bne8", "Beq8"),
    ("Bvc8", "Bvs8"),
    ("Bpl8", "Bmi8"),
    ("Bge8", "Blt8"),
    ("Bgt8", "Ble8"),
];

/// A shared machine stage over the m68k function, still in SSA form (before phi
/// elimination): the function through `to_ssa_lir`, the stage and `from_ssa`.
/// `stage` is made with the register-class oracle (`constrainRegClass`).
pub fn shared_stage(
    function: &MFunction,
    stage: impl FnOnce(
        std::rc::Rc<llrm_codegen::backend::classes::RegisterClasses>,
        llrm_codegen::backend::precse::Constrain,
    ) -> Box<dyn llrm_codegen::model::passes::LIRTransform>,
) -> Result<MFunction, String> {
    // the classes read the target's description: once per thread, not per
    // function
    thread_local! {
        static CLASSES: std::rc::Rc<llrm_codegen::backend::classes::RegisterClasses> =
            std::rc::Rc::new(llrm_codegen::backend::classes::RegisterClasses::of(&crate::target::M68k));
    }
    let classes = CLASSES.with(std::rc::Rc::clone);
    llrm_spike_mf::tolir::host_ssa::<M68kRegisters>(function, llrm_lir::registers::Regs(&REGISTER_INFO), |constrain| {
        stage(classes, constrain)
    })
}

/// The shared flow's `PhiElimination` and `TwoAddress`, in the order
/// `flow::machine_with` runs them, as the body-to-body step
/// [`allocate_greedy_after`] takes.
pub fn shared_prelude(body: llrm_codegen::model::lir::LirBody) -> Result<llrm_codegen::model::lir::LirBody, String> {
    use llrm_codegen::model::passes::LIRTransform;
    thread_local! {
        static CLASSES: std::rc::Rc<llrm_codegen::backend::classes::RegisterClasses> =
            std::rc::Rc::new(llrm_codegen::backend::classes::RegisterClasses::of(&crate::target::M68k));
    }
    let classes = CLASSES.with(std::rc::Rc::clone);
    let profile = llrm_codegen::backend::cpu::tuned_for(&crate::target::M68k, "m68000", false)?;
    let regs = llrm_lir::registers::Regs(llrm_target::Target::registers(&crate::target::M68k));
    let segments = llrm_codegen::backend::target::Segments::of(
        &llrm_target::Target::machine(&crate::target::M68k),
        regs,
        llrm_target::Target::operand_bytes(&crate::target::M68k),
    );
    let mut phases = llrm_codegen::flow::machine_with(
        &Default::default(),
        None,
        None,
        None,
        false,
        profile,
        &segments,
        &classes,
        &llrm_codegen::backend::peep::Rules::NONE,
        &llrm_target::Target::frame_registers(&crate::target::M68k),
        llrm_codegen::backend::shadow::Selector::Old,
        &[],
    )?;
    let mut body = body;
    for phase in phases.iter_mut() {
        if matches!(phase.class_name(), "MachineSink" | "PhiElimination" | "TwoAddress") {
            body = phase.transform(body)?;
        }
    }
    Ok(body)
}

/// The shared flow's `MachineCSE` over the m68k function.
pub fn shared_cse(function: &MFunction) -> Result<MFunction, String> {
    shared_stage(function, |classes, constrain| {
        Box::new(llrm_codegen::backend::precse::MachineCse { classes, keep_compare: false, constrain: Some(constrain) })
    })
}

/// The shared flow's early `MachineLICM` over the m68k function; `size` is a
/// size build.
pub fn shared_licm(
    function: &MFunction,
    size: bool,
) -> Result<MFunction, String> {
    use llrm_target::Target as _;
    let arch = crate::target::M68k;
    let segments = llrm_codegen::backend::target::Segments::of(
        &arch.machine(),
        llrm_lir::registers::Regs(arch.registers()),
        arch.operand_bytes(),
    );
    shared_stage(function, |classes, _| {
        Box::new(llrm_codegen::backend::machinelicm::MachineLicm { classes, segments, size })
    })
}

/// `BranchFolder` over the m68k function.
pub fn branch_fold(
    function: &mut MFunction,
    assignment: &Assignment,
) -> bool {
    llrm_spike_mf::branch_fold_hoisting::<M68kRegisters>(function, assignment)
}

/// `MachineBlockPlacement` over the m68k function. M68kPassConfig does not
/// override `addBlockPlacement`, so the pin runs it at every `-O` above 0.
///
/// DEVIATE M68K-TOP (with `top_saves_jumps`): a fall-through gains only where
/// it removes a jump, from a block with one successor; see
/// `llrm_spike_mf::placement` (`fall_through_gains`). Off for the pin's order.
pub fn place_blocks(
    function: &mut MFunction,
    odds: &llrm_spike_mf::placement::Odds,
    size: bool,
    top_saves_jumps: bool,
) -> bool {
    llrm_spike_mf::placement::place_blocks_priced::<_, M68kRegisters>(function, odds, size, false, top_saves_jumps)
}

pub fn remove_identity_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    llrm_spike_mf::remove_identity_copies::<M68kRegisters>(function, assignment)
}

/// An edge the phi's copy would cross a compare on gets a block of its own: a
/// DEVIATE from `SplitPHIEdges` (see the shared function) priced by
/// `tests/pin_output.rs::a_max_loop_carries_its_maximum_in_two_register_moves`.
pub fn split_flag_edges(function: &mut MFunction) {
    llrm_spike_mf::regalloc::split_flag_edges::<M68kRegisters, _>(function)
}

/// `TwoAddressInstructionPass`'s choice of the tied source.
pub fn commute_tied(function: &mut MFunction) {
    llrm_spike_mf::commute_tied::<M68kRegisters>(function)
}

/// A 32-bit value only copies and instructions that take any register touch
/// is given `XR32`: after `TwoAddress` the tied use reads the copy's
/// destination, so the source keeps the `DR32` the tied operand gave it at
/// selection for nothing.
///
/// DEVIATE C17 (the pin keeps the class: `constrainRegClass` only narrows):
/// `fib` -Os joined the add's source copy to the carried `a`, which narrowed to
/// `DR32`, and with `s` and `i` three values needed the two scratch data
/// registers: the counter took `d2` (a push and a pop, +4 B against gcc's `a0`
/// for `a`). Owner: m68k. Exit: 2026-11-28. Off for `Options::PINNED`.
pub fn relax_copy_only_classes(function: &mut MFunction) -> bool {
    let mut changed = false;
    let count = function.classes.len();
    let mut wanted = vec![Class::Xr32; count];
    let mut fixed = vec![false; count];
    let (mut defs, mut uses) = (vec![0u32; count], vec![0u32; count]);
    for operand in function.blocks.iter().flat_map(|block| &block.insts).flat_map(|inst| &inst.operands) {
        match operand {
            Operand::Def(Reg::Virt(v)) => defs[*v as usize] += 1,
            Operand::Use(Reg::Virt(v)) => uses[*v as usize] += 1,
            _ => {}
        }
    }
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if inst.kind == Kind::Copy {
            // a copy to or from a byte or word view reads that view of the
            // value, and an address register has none
            let partial = inst
                .operands
                .iter()
                .any(
                    |operand| match operand {
                        Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) => {
                            matches!(function.classes[*v as usize], Class::Dr8 | Class::Dr16)
                        }
                        Operand::Def(Reg::Phys(p)) | Operand::Use(Reg::Phys(p)) => REGISTER_INFO.root(*p) != *p,
                        _ => false,
                    },
                );
            if partial {
                for operand in &inst.operands {
                    if let Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v)) = operand {
                        fixed[*v as usize] = true;
                    }
                }
            }
            continue;
        }
        for (at, operand) in inst.operands.iter().enumerate() {
            let (Operand::Def(Reg::Virt(v)) | Operand::Use(Reg::Virt(v))) = operand else { continue };
            let v = *v as usize;
            // a move into an address register leaves the codes alone: a value a
            // flag setter defines stays in a data register
            if inst.sets_flags() && matches!(operand, Operand::Def(_)) {
                fixed[v] = true;
            }
            match inst.kind {
                Kind::Op(name) => match crate::mfunc::operand_class(name, at).and_then(|class| wanted[v].meet(class)) {
                    Some(class) => wanted[v] = class,
                    None => fixed[v] = true,
                },
                Kind::Copy => {}
            }
        }
    }
    for v in 0..count {
        if !fixed[v] && defs[v] == 1 && uses[v] == 1 && function.classes[v] == Class::Dr32 && wanted[v] == Class::Xr32 {
            function.classes[v] = Class::Xr32;
            changed = true;
        }
    }
    changed
}

/// `MachineCopyPropagation`'s spurious copies; the 8 and 16-bit classes copy
/// part of a register and do not count.
pub fn remove_redundant_copies(
    function: &mut MFunction,
    assignment: &Assignment,
) {
    llrm_spike_mf::remove_redundant_copies::<M68kRegisters>(function, assignment, |class| {
        matches!(class, Class::Dr32 | Class::Ar32 | Class::Xr32)
    })
}

impl<const ADDRESS_FIRST: bool, const ARGUMENT_REMAT: bool, const GLOBAL_COPIES: bool, const LOOP_CARRIED: bool> Jump
    for Registers<ADDRESS_FIRST, ARGUMENT_REMAT, GLOBAL_COPIES, LOOP_CARRIED>
{
    fn reverse(branch: &llrm_spike_mf::Inst) -> Option<llrm_spike_mf::Inst> {
        let Kind::Op(name) = branch.kind else { return None };
        let opposite = CONDITIONAL
            .iter()
            .find_map(
                |&(one, two)| match () {
                    _ if one == name => Some(two),
                    _ if two == name => Some(one),
                    _ => None,
                },
            )?;
        Some(llrm_spike_mf::Inst { kind: Kind::Op(opposite), ..branch.clone() })
    }

    fn jump(block: u32) -> llrm_spike_mf::Inst {
        let mut inst = llrm_spike_mf::Inst::new(Kind::Op("BRA8"), vec![Operand::Block(block)]);
        inst.terminator = true;
        inst.barrier = true;
        inst
    }
}
