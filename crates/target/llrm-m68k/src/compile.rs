//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use std::rc::Rc;

use llrm_mir::memory::has;
use llrm_mir::module::Module;

use crate::dagisel::function::select_function_with;
use crate::dagisel::hooks::Cpu;
use crate::expand::{Line, finish_with};
use crate::flags::{preserve_flags, restore_remade_flags};
use crate::machine::{M68k, M68kMachine};
use crate::mc;
use crate::mfunc::{Shape, emit_with, expand_cmov};
use crate::regalloc::{
    branch_fold, commute_tied, eliminate_phis_with, fold_reserved_copies, machine_sink, remove_identity_copies,
    remove_redundant_copies, two_address,
};

/// A line of the function: a label or an instruction.
#[derive(Clone, Debug)]
pub enum Item {
    Label(String),
    Inst(mc::Inst),
    /// Bytes placed as they are: code assembled elsewhere, or data.
    Bytes(Vec<u8>),
    /// A jump table: the absolute address of each label, as 32-bit words
    /// (`EK_BlockAddress`).
    Table(Vec<String>),
    /// The source place of the instructions that follow (`.loc`): printed only
    /// for `-g`.
    Loc(crate::expand::Row),
}

/// The pin's code model (`-mcmodel`), which decides how a global is reached
/// (`M68kSubtarget::classifyGlobalReference`, without PIC): small, PC-relative
/// with a 16-bit displacement; medium and large, absolute. The assembler
/// relaxes only branches, so data farther than 32 KiB from the code needs
/// medium or large.
///
/// DEVIATE: medium is absolute on the 68020 and later as well. The pin picks
/// PC-relative there (`M68kSubtarget::classifyLocalReference`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kSubtarget.cpp#L199-L208>),
/// but its only PC-relative data form is `(d16,%pc)` (`MxPCD`,
/// <https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/M68k/M68kInstrInfo.td#L417-L422>
/// is the indexed `(bd32,%pc,Xn)` form), so a global past 32 KiB fails to link:
/// `relocation truncated to fit: R_68K_PC16` for `memcpy_runtime`'s two
/// 70016-byte arrays (command and output in
/// `docs/working/m68k-medium-pin-pc16.md`). Reason: medium has to reach any
/// offset; `abs.l` does at every cpu, and a `(bd32,%pc)` form would be beyond
/// the pin and 2 B longer. Position independence is not modelled (`-fPIC` is
/// ignored). Owner: m68k. Review: 2026-12-15, or when the pin selects a 32-bit
/// PC-relative form for data, whichever is first.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CodeModel {
    #[default]
    Small,
    Medium,
    Large,
}

impl CodeModel {
    /// `-mcmodel=NAME`: `small`, `medium` or `large`.
    pub fn parse(name: &str) -> Result<CodeModel, String> {
        match name {
            "small" => Ok(CodeModel::Small),
            "medium" => Ok(CodeModel::Medium),
            "large" => Ok(CodeModel::Large),
            other => Err(format!("`{other}` is not a code model (small, medium, large)")),
        }
    }

    /// Whether a global's address is an absolute one.
    pub fn absolute_data(self) -> bool {
        match self {
            CodeModel::Small => false,
            CodeModel::Medium | CodeModel::Large => true,
        }
    }
}

/// What the compiler may do beyond the pin's output.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// The pin's code model: how a global's address is formed
    /// ([`CodeModel::absolute_data`]).
    pub code_model: CodeModel,
    /// Run the shared IR mid-end (`-O2`) first, as `llc -O2` is given
    /// optimized IR by `opt`.
    pub mid_end: bool,
    /// Size is wanted over clocks in every function (`-Os`; the x86 path's
    /// `Cpu::size`). A function marked `optsize` or `minsize` wants it too.
    pub size: bool,
    /// At a speed level a choice takes a candidate only when it is no larger
    /// and no slower (`-O1`); otherwise it is priced by the CPU's byte rate.
    pub strict: bool,
    /// Schedule in source order (`-pre-RA-sched=source`) rather than with the
    /// ILP list scheduler `llc` defaults to.
    pub source_order: bool,
    /// Form `(An)+` and `-(An)` accesses, which gcc does and the pin does not.
    pub indexed: bool,
    /// Push and pop the saved registers where that is no larger and no
    /// slower than the pin's `suba.l` and `movem` into slots.
    pub push_frame: bool,
    /// Outgoing arguments as pushes where that is cheaper than stores into the
    /// reserved call area ([`crate::callpush`]).
    pub push_arguments: bool,
    /// `addq` and `subq` for an immediate of 1 to 8, which the pin has no
    /// instruction for.
    pub short: bool,
    /// A `tst` is dropped past instructions that set the codes between it and
    /// the definition of its register, which move above the definition
    /// ([`crate::cmpelim`]).
    pub hoist_flag_setters: bool,
    /// `cmpi.l #(2^s-1)` after the `lsr.l #s` of the same value is dropped
    /// ([`crate::cmpshift`]).
    pub reuse_shift_flags: bool,
    /// A `tst` of the register an `add` of it to itself just doubled reads the
    /// carry the `add` left instead ([`crate::cmpcarry`]).
    pub reuse_shift_out_flags: bool,
    /// A load and the step of its pointer in another block are one `(An)+`
    /// ([`crate::loopinc`]).
    pub post_inc_across_blocks: bool,
    /// A spill slot lives in a free address register where one is free over
    /// its range (DEVIATE SLOT-REG, [`crate::slotreg`]). `LLRM_SLOT_REG` sets
    /// it.
    pub slot_registers: bool,
    /// The shared flow's `MachineCSE` runs on the function in SSA form, before
    /// phi elimination ([`crate::regalloc::shared_cse`]). A function it
    /// changed is also built without it and the cheaper kept
    /// ([`crate::trial`]): shared values live longer, and across calls they
    /// spill (11-12 corpus: 222 cells larger, 136 smaller, without the trial).
    pub shared_cse: bool,
    /// A value read in a block after an earlier one defines it is read from its
    /// register, as in the pin's MIR: what the hosted stages (MachineLICM) need
    /// to see it as loop-invariant ([`crate::mfunc::emit_with`]).
    pub direct_copies: bool,
    /// The shared flow's early `MachineLICM` runs on the function in SSA form,
    /// before `MachineCSE` ([`crate::regalloc::shared_licm`]). It sees a
    /// loop-invariant only where `direct_copies` made no copy of it.
    pub shared_licm: bool,
    /// The shared flow's `PhiElimination` and `TwoAddress` take the function
    /// from SSA to the allocator, in place of the private `eliminate_phis`,
    /// `commute_tied` and `two_address` ([`crate::regalloc::shared_prelude`]).
    /// Off: the private passes are what the corpus was tuned on.
    pub shared_prelude: bool,
    /// A small conditional block is copied into the predecessors that jump to
    /// it, inside a loop ([`llrm_spike_mf::taildup`]); not in a size build, and
    /// kept only where the build with the copies beats the build without.
    pub tail_duplicate: bool,
    /// A load of an incoming stack argument is made again where it is read
    /// ([`crate::regalloc::Registers`], M68K-ARGREMAT).
    pub rematerialize_arguments: bool,
    /// How a size-built function chooses between an inline expansion and a call
    /// to the runtime ([`crate::libcalls`], M68K-SIZE-LIBCALL).
    pub size_libcalls: crate::libcalls::Mode,
    /// The kinds that choice made for the module, as [`crate::libcalls::mask`];
    /// none given, the module is scanned for the function.
    pub libcalls: Option<u8>,
    /// A loop's top is chosen by the jumps its fall-throughs remove, not by
    /// the edges they cover (M68K-TOP, [`crate::regalloc::place_blocks`]).
    pub loop_top_by_jumps: bool,
    /// A callee called again and again is called through an address register
    /// where that makes the function cheaper ([`crate::calltarget`]).
    pub call_targets: bool,
    /// A call of a function this module defines is `jsr (name,%pc)`, 4 B for
    /// the 6 of `jsr name`. Only for a driver that has checked the module's
    /// code is within 16-bit reach of itself ([`crate::driver`]).
    pub pc_calls: bool,
    /// A sum in an address register adds a sign-extended word with `adda.w`
    /// ([`crate::sextacc`]).
    pub sext_adds: bool,
    /// A block that only returns is copied into the predecessors that go only
    /// there (`TailDuplicator`); kept when the function is cheaper for it.
    pub return_dup: bool,
    /// A value an address register would hold across a loop it is not used in
    /// is held in a data register (`crate::classsplit`); kept when cheaper.
    pub class_split: bool,
    /// The destination of a long compare may be an address register
    /// (`cmpa.l`, [`crate::addrcmp`]); kept when the function is cheaper.
    pub compare_destinations: bool,
    /// A 64-bit add or subtract is `add`/`addx` (`ADDC`/`ADDE`, the pin's
    /// lowering); a function it changed is also built with the compare form
    /// and the cheaper kept: `addx` needs data registers on both halves.
    pub carry_chain: bool,
    /// The coalescer joins the copies of values that live in more than one
    /// block with the ones that do not (`JoinGlobalCopies`, the allocator's
    /// tuning); a function that saved a register is also built without, and
    /// the cheaper kept. False is the pin's M68k.
    pub global_copies: bool,
    /// A pass-through copy temp takes either register kind
    /// ([`crate::regalloc::relax_copy_only_classes`], DEVIATE C17); a function
    /// that saved a callee-saved register is also built with it, and the
    /// cheaper kept.
    pub relax_classes: bool,
    /// Make a `moveq` constant again where it is read ([`crate::moveqremat`]).
    pub remat_constants: bool,
    /// Multiplies by `(2^n +- 1) << k` (DEVIATE M68K-MUL-LATE,
    /// [`crate::dagisel::mulprice`]).
    pub mul: crate::dagisel::mulprice::Mode,
    /// The coalescer tries first the block whose copies read a value carried
    /// around the loop (`joins_loop_carried_first`, DEVIATE C15); false is the
    /// pin's block order.
    pub loop_carried_first: bool,
    /// Print the source places (`.loc`) of the instructions (`-g`).
    pub debug: bool,
}

impl Options {
    /// The pin's output, instruction for instruction: what `llc` and `llvm-mc`
    /// can be compared with.
    pub const PINNED: Options = Options {
        code_model: CodeModel::Small,
        mid_end: false,
        size: false,
        size_libcalls: crate::libcalls::Mode::Auto,
        libcalls: None,
        strict: false,
        source_order: false,
        indexed: false,
        push_frame: false,
        push_arguments: false,
        short: false,
        hoist_flag_setters: false,
        reuse_shift_flags: false,
        reuse_shift_out_flags: false,
        post_inc_across_blocks: false,
        slot_registers: false,
        shared_cse: false,
        direct_copies: false,
        shared_licm: false,
        shared_prelude: false,
        tail_duplicate: false,
        rematerialize_arguments: false,
        loop_top_by_jumps: false,
        call_targets: false,
        pc_calls: false,
        sext_adds: false,
        return_dup: false,
        class_split: false,
        compare_destinations: false,
        carry_chain: true,
        global_copies: true,
        relax_classes: false,
        remat_constants: false,
        mul: crate::dagisel::mulprice::Mode::Pin,
        loop_carried_first: false,
        debug: false,
    };
}

impl Default for Options {
    fn default() -> Options {
        Options {
            code_model: CodeModel::Small,
            mid_end: true,
            size: false,
            size_libcalls: crate::libcalls::Mode::Auto,
            libcalls: None,
            strict: false,
            source_order: false,
            indexed: true,
            push_frame: true,
            push_arguments: true,
            short: true,
            hoist_flag_setters: true,
            reuse_shift_flags: true,
            reuse_shift_out_flags: true,
            post_inc_across_blocks: true,
            slot_registers: llrm_support::env_flag!("LLRM_SLOT_REG"),
            shared_cse: true,
            direct_copies: false,
            shared_licm: false,
            shared_prelude: false,
            tail_duplicate: true,
            rematerialize_arguments: true,
            loop_top_by_jumps: true,
            call_targets: true,
            pc_calls: false,
            sext_adds: true,
            return_dup: true,
            class_split: true,
            compare_destinations: true,
            carry_chain: true,
            global_copies: true,
            relax_classes: true,
            remat_constants: true,
            mul: crate::dagisel::mulprice::Mode::Trial,
            loop_carried_first: true,
            debug: false,
        }
    }
}

/// The function after selection, allocation and expansion: its label, then
/// each block's label where a branch names it, and the instructions.
pub fn items(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<Vec<Item>, String> {
    items_with(module, name, cpu, Options::default())
}

/// [`items`] with `options`.
///
/// A function an optional rewrite touched (the `adda`/`suba` fold, the copy of
/// a return block into its predecessors) is also built without it, and the
/// cheaper kept (`crate::trial`): each is priced before the code is known. A
/// function that saved a register is last built with the pin's join order,
/// which replaces the tuned one only when cheaper.
///
/// A function that saved a callee-saved register is also built once more with
/// the class relaxation ([`Options::relax_classes`]) on the choices the trials
/// made, and the relaxed build kept only where cheaper: the relaxation exists
/// to free a scratch register for a value, so a build that saves none has
/// nothing to gain. One extra build, not the sequence again: the sequence
/// cost a corpus 1.64 times the instructions, one file twice.
pub fn items_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<Vec<Item>, String> {
    let optimized;
    let module = if options.mid_end {
        let narrow = !options.strict && !size_build(module, name, options);
        optimized = mid_end_narrowing(module, &M68k::for_cpu(cpu), Some(Rc::new(M68kMachine { cpu })), narrow)?;
        &optimized
    } else {
        module
    };
    let options = Options { mid_end: false, global_copies: options.global_copies, ..options };
    let wants_size = size_build(module, name, options);
    let price = crate::timing::Price::of(wants_size, options.strict);
    // The relaxation is the last choice of a build, so it is made before the
    // two builds of the shared `MachineCSE` are compared.
    let prefixes = Prefixes::default();
    let finished = |options: Options| -> Result<(Vec<Item>, bool), String> {
        let (plain, saved, chosen, shared) =
            items_trials(module, name, cpu, Options { relax_classes: false, ..options }, &prefixes)?;
        if !options.relax_classes || !saved {
            return Ok((plain, shared));
        }
        let (relaxed, touched) = items_once(module, name, cpu, Options { relax_classes: true, ..chosen }, &prefixes)?;
        let relax = touched.relax_classes && crate::trial::keeps(&relaxed, &plain, cpu, price);
        Ok((if relax { relaxed } else { plain }, shared))
    };
    let (with, shared) = finished(options)?;
    // The shared `MachineCSE` lengthens live ranges, and across calls they
    // spill. A function it changed is built through the whole sequence both
    // ways (the other trials' choices depend on the shape), the cheaper kept.
    if shared {
        let (without, _) = finished(Options { shared_cse: false, ..options })?;
        if !crate::trial::keeps(&with, &without, cpu, price) {
            return Ok(without);
        }
    }
    Ok(with)
}

/// The trials of [`items_with`] on a module the mid-end has run on: the items,
/// whether the first build saved a callee-saved register, and the options the
/// trials ended on.
fn items_trials(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
    prefixes: &Prefixes,
) -> Result<(Vec<Item>, bool, Options, bool), String> {
    let mut options = options;
    let (mut kept, touched) = items_once(module, name, cpu, options, prefixes)?;
    let saved = touched.global_copies;
    if llrm_support::debug::enabled("builds") {
        eprintln!(
            "[builds] {name} trials: tail_dup {} return_dup {} class_split {} compare_destinations {} carry_chain {} global_copies {} mul_trial {} remat_constants {} shared_cse {}",
            touched.tail_dup,
            touched.return_dup,
            touched.class_split,
            touched.compare_destinations,
            touched.carry_chain,
            touched.global_copies,
            touched.mul_trial,
            touched.remat_constants,
            touched.shared_cse
        );
    }
    if !touched.any() {
        return Ok((kept, saved, options, touched.shared_cse));
    }
    let wants_size = size_build(module, name, options);
    let price = crate::timing::Price::of(wants_size, options.strict);
    // first: the multiply decides the shape of the rest, and its price counts
    // the library routine, which the other rewrites do not touch
    if touched.mul_trial {
        use crate::dagisel::mulprice::Mode;
        let without = Options { mul: Mode::Kept, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if touched.mul_synthesized {
            // DEVIATE G2: the pin's shapes are judged as before (clocks
            // count wherever they run); a chain beyond them only against
            // that winner, and its clocks only inside loops
            let shapes = Options { mul: Mode::TrialPinShapes, ..options };
            let (pin, _) = items_once(module, name, cpu, shapes, prefixes)?;
            let (base, base_options) = if crate::trial::prefers_without(&pin, &plain, cpu, price, true) {
                (plain, without)
            } else {
                (pin, shapes)
            };
            if crate::trial::prefers_without(&kept, &base, cpu, price, false) {
                kept = base;
                options = base_options;
            }
        } else if crate::trial::prefers_without(&kept, &plain, cpu, price, true) {
            kept = plain;
            options = without;
        }
    }
    if touched.class_split {
        let without = Options { class_split: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.tail_dup {
        let without = Options { tail_duplicate: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.return_dup {
        let without = Options { return_dup: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.global_copies {
        // the default stands unless the other join order is cheaper
        let other = Options { global_copies: false, ..options };
        let (items, _) = items_once(module, name, cpu, other, prefixes)?;
        if crate::trial::keeps(&items, &kept, cpu, price) {
            kept = items;
            options = other;
        }
    }
    if touched.remat_constants {
        let without = Options { remat_constants: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    // the address registers it takes may be the ones a later choice (the call
    // target's) would have had, so it is judged with the rest built
    if touched.slot_registers {
        let without = Options { slot_registers: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    // last: the two builds compared are final, the other choices made
    if touched.carry_chain {
        let without = Options { carry_chain: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.compare_destinations {
        let without = Options { compare_destinations: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without, prefixes)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
        }
    }
    Ok((kept, saved, options, touched.shared_cse))
}

/// Whether `function` carries through the extend flag: an `addx`, `subx` or
/// `negx` (a subtract from a constant zero high half is `negx`).
/// A tied def and use share a register once allocated: a write-back form such
/// as `add.w (An)+, Dn` is printed from its use, so a def allocated elsewhere
/// would leave the pointer it should have moved where it was.
fn verify_ties(
    function: &crate::mfunc::MFunction,
    assignment: &llrm_spike_mf::regalloc::Assignment,
) -> Result<(), String> {
    let root = |reg: &llrm_spike_mf::Reg| match reg {
        llrm_spike_mf::Reg::Virt(v) => crate::REGISTER_INFO.root(assignment.0[*v as usize]),
        llrm_spike_mf::Reg::Phys(id) => crate::REGISTER_INFO.root(*id),
    };
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        for &(def, used) in &inst.ties {
            if let (llrm_spike_mf::Operand::Def(def), llrm_spike_mf::Operand::Use(used)) =
                (&inst.operands[def], &inst.operands[used])
                && root(def) != root(used)
            {
                return Err(format!(
                    "{}: {:?} ties a def and a use allocated to different registers",
                    function.name, inst.kind
                ));
            }
        }
    }
    Ok(())
}

pub fn uses_extend(function: &crate::mfunc::MFunction) -> bool {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.insts)
        .any(
            |inst| matches!(
                inst.kind,
                llrm_spike_mf::Kind::Op(name) if ["ADDX", "SUBX", "NEGX"].iter().any(|row| name.starts_with(row))
            ),
        )
}

/// The optional rewrites a build used.
#[derive(Clone, Copy, Default)]
struct Touched {
    /// A block was copied into the arms that jump to it
    /// ([`llrm_spike_mf::taildup::tail_duplicate`]).
    tail_dup: bool,
    return_dup: bool,
    class_split: bool,
    /// A compare's destination was left to the allocator's choice of an
    /// address or a data register ([`crate::addrcmp`]).
    compare_destinations: bool,
    /// The function has an `addx`, `subx` or `negx` ([`uses_extend`]).
    carry_chain: bool,
    /// The build saved a callee-saved register: the join order may have cost
    /// it.
    global_copies: bool,
    /// The build decomposed a multiply ([`crate::dagisel::mulprice::Chain`]).
    mul_trial: bool,
    /// The build widened the class of a pass-through copy temp.
    relax_classes: bool,
    /// The build marked a `moveq` constant rematerializable
    /// ([`crate::moveqremat`]).
    remat_constants: bool,
    /// The shared `MachineCSE` changed the function.
    shared_cse: bool,
    /// A spill slot moved into an address register ([`crate::slotreg`]).
    slot_registers: bool,
    /// ... and one of them was a chain beyond the pin's shape (DEVIATE G2): the
    /// trial credits clocks only inside loops, since a straight-line multiply
    /// may never run.
    mul_synthesized: bool,
}

impl Touched {
    fn any(self) -> bool {
        self.tail_dup
            || self.return_dup
            || self.class_split
            || self.compare_destinations
            || self.carry_chain
            || self.global_copies
            || self.mul_trial
            || self.remat_constants
            || self.slot_registers
    }
}

/// Whether `name` is built for size: the level, or an `optsize`/`minsize`
/// attribute.
fn size_build(
    module: &Module,
    name: &str,
    options: Options,
) -> bool {
    options.size
        || module
            .named(name)
            .and_then(|id| module.global(id).function())
            .is_some_and(|one| has(&one.attrs, "optsize") || has(&one.attrs, "minsize"))
}

fn same_instructions(
    a: &crate::mfunc::MFunction,
    b: &crate::mfunc::MFunction,
) -> bool {
    a.blocks.len() == b.blocks.len() && a.blocks.iter().zip(&b.blocks).all(|(a, b)| a.insts == b.insts)
}

/// The bytes of `items` as assembled (a branch not yet sized counts 6).
pub fn items_bytes(items: &[Item]) -> usize {
    items
        .iter()
        .map(|item| match item {
            Item::Inst(inst) => crate::encode::encode(inst).map_or(6, |one| one.bytes.len()),
            Item::Bytes(bytes) => bytes.len(),
            Item::Table(labels) => 4 * labels.len(),
            _ => 0,
        })
        .sum()
}

/// The runtime calls the size-built functions of `module` make, as a
/// [`crate::libcalls::mask`]: chosen once for the module (M68K-SIZE-LIBCALL),
/// by the bytes each function builds to with and without them.
pub fn libcalls_for(
    module: &Module,
    cpu: Cpu,
    options: Options,
) -> u8 {
    let kinds = crate::libcalls::chosen(
        module,
        cpu,
        options.size_libcalls,
        |one| options.size || has(&one.attrs, "optsize") || has(&one.attrs, "minsize"),
        |name, kinds| {
            // The price is one build: a call against a loop differs by far more than a
            // trial moves, and each trial set costs several builds (x2.65 compile time).
            let options = Options {
                libcalls: Some(crate::libcalls::mask(kinds)),
                shared_cse: false,
                tail_duplicate: false,
                return_dup: false,
                class_split: false,
                compare_destinations: false,
                global_copies: false,
                relax_classes: false,
                remat_constants: false,
                slot_registers: false,
                ..options
            };
            items_with(module, name, cpu, options).map(|items| items_bytes(&items))
        },
    );
    crate::libcalls::mask(&kinds)
}

/// Everything that decides what selection and emission make of a function: the
/// trials of [`items_with`] that change none of it share one result.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Selection {
    source_order: bool,
    indexed: bool,
    short: bool,
    absolute_data: bool,
    wants_size: bool,
    strict: bool,
    carry_chain: bool,
    mul: crate::dagisel::mulprice::Mode,
    libcalls: Vec<crate::libcalls::Kind>,
    direct_copies: bool,
    copy_destinations: bool,
}

thread_local! {
    static RUNS: std::cell::Cell<Runs> = const { std::cell::Cell::new(Runs { builds: 0, selections: 0, early: 0, late: 0 }) };
}

/// How often this thread ran each stage of a build: the regression tests of
/// the shared stages count them.
#[derive(Clone, Copy, Debug)]
pub struct Runs {
    pub builds: usize,
    pub selections: usize,
    /// The rewrites up to the shared `MachineCSE`.
    pub early: usize,
    /// The rewrites from `MachineSink` to allocation.
    pub late: usize,
}

pub fn runs() -> Runs {
    RUNS.with(std::cell::Cell::get)
}

fn counted(count: impl FnOnce(&mut Runs)) {
    RUNS.with(|runs| {
        let mut now = runs.get();
        count(&mut now);
        runs.set(now);
    });
}

/// The function as emitted, with whether the build decomposed a multiply and
/// took a chain beyond the pin's shape.
type Emission = (crate::mfunc::MFunction, bool, bool);

/// A result kept by the inputs that made it.
struct Memo<K, V>(std::cell::RefCell<llrm_support::hash::HashMap<K, V>>);

impl<K, V> Default for Memo<K, V> {
    fn default() -> Self {
        Memo(Default::default())
    }
}

impl<K: Eq + std::hash::Hash, V: Clone> Memo<K, V> {
    fn get_or(
        &self,
        key: K,
        make: impl FnOnce() -> Result<V, String>,
    ) -> Result<V, String> {
        if let Some(found) = self.0.borrow().get(&key) {
            return Ok(found.clone());
        }
        let made = make()?;
        self.0.borrow_mut().insert(key, made.clone());
        Ok(made)
    }
}

/// The function before allocation, with what its rewrites touched.
type Prepared = (crate::mfunc::MFunction, Touched);

/// What the builds of one function share, stage by stage. Each key is the
/// inputs the stage and those before it read, so a trial that changes only a
/// later option takes the earlier stages' result: selection and emission (a
/// third of a build; only the multiply and carry-chain trials change its
/// inputs), the rewrites up to the shared `MachineCSE`, and the rest of
/// the rewrites before allocation. The options as text: a new option is in
/// every key until it is taken out below.
#[derive(Default)]
struct Prefixes {
    emitted: Memo<Selection, Emission>,
    early: Memo<(Selection, String), Prepared>,
    late: Memo<(Selection, String), Prepared>,
}

/// The options that only the stages after `MachineCSE` read, left out of the
/// key of the stage before it.
fn before_late_stage(options: Options) -> Options {
    Options { relax_classes: false, remat_constants: false, ..before_allocation(options) }
}

/// The options that only allocation and the stages after it read.
fn before_allocation(options: Options) -> Options {
    Options { global_copies: false, tail_duplicate: false, return_dup: false, ..options }
}

/// One build of the function, and the optional rewrites that changed it.
fn items_once(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
    prefixes: &Prefixes,
) -> Result<(Vec<Item>, Touched), String> {
    // one full select, allocate and emit; a function with optional rewrites
    // runs it again per trial
    counted(|runs| runs.builds += 1);
    let _build = llrm_support::debug::span("m68k build");
    if llrm_support::debug::enabled("builds") {
        eprintln!("[builds] {name}");
    }
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let number =
        module.functions().filter(|(_, _, one)| !one.is_declaration()).position(|(one, _, _)| one == id).unwrap_or(0);
    let wants_size = options.size
        || module.global(id).function().is_some_and(|one| has(&one.attrs, "optsize") || has(&one.attrs, "minsize"));
    // DEVIATE M68K-SIZE-LIBCALL (`crate::libcalls`): the 68000 and 68010 always
    // call the counts; their variable shifts are chosen like the 68020's
    let libcalls = if wants_size {
        let mut kinds =
            crate::libcalls::from_mask(options.libcalls.unwrap_or_else(|| libcalls_for(module, cpu, options)));
        kinds.retain(|&kind| crate::libcalls::decided_at(kind, cpu));
        kinds
    } else {
        Vec::new()
    };
    let selection = Selection {
        source_order: options.source_order,
        indexed: options.indexed,
        short: options.short,
        absolute_data: options.code_model.absolute_data(),
        wants_size,
        strict: options.strict,
        carry_chain: options.carry_chain,
        mul: options.mul,
        libcalls: libcalls.clone(),
        direct_copies: options.direct_copies,
        // what the hosted stages read: a result is defined in its copy's
        // register
        copy_destinations: options.direct_copies || options.shared_cse || options.shared_licm,
    };
    let (emitted, mul_used, mul_synthesized) = prefixes
        .emitted
        .get_or(
            selection.clone(),
            || {
                let selection = &selection;
                counted(|runs| runs.selections += 1);
                let chain = crate::dagisel::mulprice::Chain::of(selection.mul);
                let blocks = {
                    let _span = llrm_support::debug::span("m68k select");
                    select_function_with(
                        module,
                        name,
                        cpu,
                        selection.source_order,
                        selection.indexed,
                        selection.short,
                        selection.absolute_data,
                        selection.wants_size,
                        selection.carry_chain,
                        crate::timing::Price::of(selection.wants_size, selection.strict),
                        &chain,
                        &selection.libcalls,
                    )?
                };
                let _span = llrm_support::debug::span("m68k emit");
                let function = emit_with(
                    name,
                    &blocks,
                    Shape { direct_copies: selection.direct_copies, copy_destinations: selection.copy_destinations },
                )?;
                Ok((function, chain.used(), chain.synthesized()))
            },
        )?;
    let sp = llrm_support::debug::span("m68k prealloc");
    let (function, touched) =
        prefixes.early.get_or((selection.clone(), format!("{:?}", before_late_stage(options))), || {
            counted(|runs| runs.early += 1);
            let mut function = emitted.clone();
            let mut touched = Touched::default();
            touched.carry_chain = options.carry_chain && uses_extend(&function);
            touched.mul_trial = options.mul == crate::dagisel::mulprice::Mode::Trial && mul_used;
            touched.mul_synthesized = mul_synthesized;
            function.returns_twice =
                module.global(id).function().is_some_and(|one| llrm_mir::memory::calls_returns_twice(module, one));
            if options.short {
                crate::cmpelim::eliminate_tests(
                    &mut function,
                    options.hoist_flag_setters,
                    options.rematerialize_arguments,
                );
                if options.reuse_shift_flags {
                    crate::cmpshift::reuse_shift_flags(&mut function);
                }
                crate::shiftadd::shifts_to_adds(&mut function);
                crate::negadd::negative_adds_to_subq(&mut function);
                crate::addrroles::address_roles(&mut function);
                crate::addrarith::address_adds(&mut function, cpu, wants_size, options.compare_destinations);
                crate::foldload::fold_loads(&mut function);
                crate::divrem::remainders_from_quotients(&mut function);
                if wants_size {
                    crate::foldarg::fold_argument_loads(&mut function);
                }
                if options.compare_destinations {
                    touched.compare_destinations = crate::addrcmp::widen_compare_destinations(&mut function) > 0;
                }
                // priced inside the pass (`Price`), so a function it changed is
                // not built again
                if options.sext_adds {
                    crate::sextacc::sign_extended_words(
                        &mut function,
                        cpu,
                        crate::timing::Price::of(wants_size, options.strict),
                    );
                }
                if options.class_split {
                    touched.class_split = crate::classsplit::hold_in_data_registers(&mut function) > 0;
                }
                if cpu.at_least(Cpu::M68020) {
                    crate::scaledindex::scale_indexes(&mut function);
                }
                if options.indexed && options.post_inc_across_blocks {
                    crate::loopinc::post_increment_across_blocks(&mut function);
                }
            }
            function.odds = Some(edge_odds(module, name)?);
            expand_cmov(&mut function)?;
            if options.shared_licm {
                function = crate::regalloc::shared_licm(&function, wants_size)?;
            }
            if options.shared_cse {
                let shared = crate::regalloc::shared_cse(&function)?;
                touched.shared_cse = !same_instructions(&function, &shared);
                function = shared;
            }
            Ok((function, touched))
        })?;
    let (mut function, mut touched) =
        prefixes.late.get_or((selection, format!("{:?}", before_allocation(options))), || {
            counted(|runs| runs.late += 1);
            let (mut function, mut touched) = (function, touched);
            machine_sink(&mut function);
            if options.short && options.reuse_shift_out_flags {
                crate::cmpcarry::reuse_shift_out_flags(&mut function);
            }
            crate::regalloc::split_flag_edges(&mut function);
            if !options.shared_prelude {
                eliminate_phis_with(&mut function, false);
            }
            fold_reserved_copies(&mut function);
            if options.short {
                crate::commutative::mark_commutative(&mut function, wants_size);
            }
            if !options.shared_prelude {
                commute_tied(&mut function);
                two_address(&mut function);
            }
            if options.indexed && options.relax_classes && !options.shared_prelude {
                touched.relax_classes = crate::regalloc::relax_copy_only_classes(&mut function);
            }
            if options.remat_constants {
                touched.remat_constants = crate::moveqremat::rematerializable_constants(&mut function) > 0;
            }
            Ok((function, touched))
        })?;
    drop(sp);
    // spill slots are numbered after the static allocas
    let first_slot = function.locals.iter().map(|local| local.0 + 1).max().unwrap_or(0);
    let allocating = llrm_support::debug::span("m68k allocate");
    let (assignment, slots) = if std::env::var_os("LLRM_M68K_SPIKE").is_some() {
        crate::regalloc::allocate_spilling_by_frequency(&mut function, first_slot)?
    } else {
        crate::regalloc::allocate_greedy_after(
            &mut function,
            wants_size,
            options.indexed,
            options.rematerialize_arguments,
            options.global_copies,
            options.loop_carried_first,
            options.shared_prelude.then_some(&crate::regalloc::shared_prelude as llrm_spike_mf::tolir::Prelude),
        )?
    };
    drop(allocating);
    let mut slots: Vec<i32> = slots.into_iter().map(|(slot, _)| slot).collect();
    let sp = llrm_support::debug::span("m68k postalloc");
    verify_ties(&function, &assignment)?;
    touched.global_copies = options.global_copies && crate::calltarget::saves_a_register(&function, &assignment);
    if options.short {
        crate::addrarith::refine_lea_round_trips(&mut function, &assignment, cpu, wants_size);
        crate::addrarith::refine_address_adds(&mut function, &assignment, cpu, wants_size);
        crate::chain::post_increment_chains(&mut function, &assignment, cpu, wants_size);
        crate::addrcmp::compare_in_address_register(&mut function, &assignment);
        crate::addrcmp::compare_address_registers(&mut function, &assignment);
        crate::addrcmp::word_immediates(&mut function);
        crate::narrow::merge_logic_immediates(&mut function, &assignment);
        crate::narrow::narrow_logic_immediates(&mut function, &assignment);
        crate::consts::materialize_constants(&mut function, &assignment, cpu, wants_size);
        crate::constreg::constant_into_destination(&mut function, &assignment);
        crate::constreg::immediate_from_free_register(&mut function, &assignment);
        crate::narrow_add::narrow_adds(&mut function, &assignment);
        crate::narrow_add::narrow_compares(&mut function, &assignment);
    }
    remove_identity_copies(&mut function, &assignment);
    remove_redundant_copies(&mut function, &assignment);
    if options.hoist_flag_setters {
        crate::cmpelim::eliminate_adjacent_tests(&mut function, &assignment);
    }
    llrm_spike_mf::spill::remove_dead_slot_stores::<crate::regalloc::M68kRegisters>(
        &mut function,
        &assignment,
        &crate::REGISTER_INFO,
        &slots,
    );
    if options.short && options.slot_registers {
        let moved = crate::slotreg::slots_in_address_registers(&mut function, &assignment, &slots);
        touched.slot_registers = !moved.is_empty();
        slots.retain(|slot| !moved.contains(slot));
    }
    if options.short {
        crate::cmpdup::drop_repeated_compares(&mut function, &assignment);
        crate::spillmove::spills_as_moves(
            &mut function,
            &assignment,
            cpu,
            crate::timing::Price::of(wants_size, options.strict),
        );
    }
    preserve_flags(&mut function, &assignment)?;
    restore_remade_flags(&mut function);
    branch_fold(&mut function, &assignment);
    if !wants_size {
        if options.tail_duplicate {
            touched.tail_dup =
                llrm_spike_mf::taildup::tail_duplicate::<_, crate::regalloc::M68kRegisters>(&mut function, 2);
        }
        if options.return_dup {
            touched.return_dup = llrm_spike_mf::taildup::duplicate_returns(&mut function, 2);
        }
    }
    crate::regalloc::place_blocks(&mut function, &edge_odds(module, name)?, wants_size, options.loop_top_by_jumps);
    drop(sp);
    let sp = llrm_support::debug::span("m68k expand");
    let mut out = vec![Item::Label(name.to_owned())];
    // The line the function is declared at starts its rows (the prologue is
    // from no instruction).
    if let Some(line) = module.subprogram_line(module.global(id).function().ok_or("not a function")?) {
        out.push(Item::Loc(crate::expand::Row {
            place: llrm_codegen::model::lir::SrcLoc::line(line),
            block_top: false,
            prologue_end: false,
            epilogue_begin: false,
        }));
    }
    let price = crate::timing::Price::of(wants_size, options.strict);
    let expand = |function: &crate::mfunc::MFunction| {
        finish_with(
            function,
            &assignment,
            number,
            &slots,
            cpu,
            options.push_frame,
            options.short,
            options.push_arguments.then_some(price),
        )
    };
    if options.short && options.pc_calls {
        crate::pccall::local_calls(&mut function, |callee| is_local_callee(module, callee));
    }
    if options.short && options.call_targets {
        let chosen = crate::calltarget::choose(&function, &assignment, cpu, price, expand)?;
        crate::calltarget::through_registers(&mut function, &chosen)?;
    }
    for line in expand(&function)? {
        out.push(match line {
            Line::Label(at) => Item::Label(format!(".LBB{number}_{at}")),
            Line::Inst(inst) => Item::Inst(inst),
            Line::Loc(place) => Item::Loc(place),
        });
    }
    out.extend(jump_tables(&function, number));
    drop(sp);
    Ok((out, touched))
}

/// Each jump table the function dispatches through: its label, then the
/// address of each block it goes to (`EK_BlockAddress`).
///
/// DEVIATE: the table follows the function in the code section; the pin puts
/// it in `.rodata`. The bytes are the same and the object writers place one
/// section. Exit: the data sections of the object writers take code labels.
fn jump_tables(
    function: &crate::mfunc::MFunction,
    number: usize,
) -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen = llrm_support::hash::HashSet::default();
    for inst in function.blocks.iter().flat_map(|block| &block.insts) {
        if inst.kind != crate::mfunc::Kind::Op("JMP32j") {
            continue;
        }
        let Some(crate::mfunc::Operand::Symbol(table, _)) =
            inst.operands.iter().find(|operand| matches!(operand, crate::mfunc::Operand::Symbol(..)))
        else {
            continue;
        };
        if !seen.insert(table.clone()) {
            continue;
        }
        let labels = inst
            .operands
            .iter()
            .filter_map(|operand| match operand {
                crate::mfunc::Operand::Block(id) => function.blocks.iter().position(|block| block.id == *id),
                _ => None,
            })
            .map(|at| format!(".LBB{number}_{at}"))
            .collect();
        out.push(Item::Label(table.clone()));
        out.push(Item::Table(labels));
    }
    out
}

/// Whether `callee` is a function `module` defines that no other definition
/// replaces at link time: a weak or linkonce one can be.
fn is_local_callee(
    module: &Module,
    callee: &str,
) -> bool {
    module
        .named(callee)
        .is_some_and(
            |id| {
                let global = module.global(id);
                global.function().is_some_and(|one| !one.is_declaration())
                    && !matches!(
                        llrm_mir::image::Export::of(global.linkage),
                        llrm_mir::image::Export::Weak
                    )
            },
        )
}

/// The IR's branch probabilities by `(from, to)` block number
/// (`BranchProbabilityInfo`) and where `to` is among `from`'s successors, for
/// block placement.
fn edge_odds(
    module: &Module,
    name: &str,
) -> Result<llrm_spike_mf::placement::Odds, String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let function = module.global(id).function().ok_or_else(|| format!("@{name} is a declaration"))?;
    let text = module.datalayout.as_deref().ok_or("a module with no datalayout")?;
    let layout = llrm_mir::datalayout::DataLayout::parse(text)?;
    let bpi = llrm_analysis::bpi::BranchProbabilityInfo::of(
        llrm_analysis::bpi::Module {
            context: &module.context,
            declarations: &module.globals,
            metadata: &module.metadata,
            layout: &layout,
        },
        function,
    );
    let mut odds = llrm_spike_mf::placement::Odds::default();
    for &block in function.layout() {
        for (index, to) in llrm_mir::loopinfo::children(function, block).enumerate() {
            let probability = bpi.get_edge_probability(function, block, index);
            let entry =
                odds.entry((block.0, to.0)).or_insert((llrm_mir::blockfrequency::BranchProbability::get_zero(), index));
            entry.0 = entry.0 + probability;
        }
    }
    Ok(odds)
}

/// The function as `llc` prints it, less the directives: the label, then each
/// instruction after a tab, and a block's label where a branch names it.
pub fn text(
    module: &Module,
    name: &str,
    cpu: Cpu,
) -> Result<String, String> {
    text_with(module, name, cpu, Options::default())
}

/// [`text`] with `options`.
pub fn text_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<String, String> {
    printed(items_with(module, name, cpu, options)?, options.debug)
}

/// `items` as assembly text, the `.loc` rows only with `debug`.
pub fn printed(
    items: Vec<Item>,
    debug: bool,
) -> Result<String, String> {
    let mut out = String::new();
    let mut previous = None;
    for item in items {
        match item {
            Item::Label(label) => out.push_str(&format!("{label}:\n")),
            Item::Inst(inst) => out.push_str(&format!("\t{}\n", mc::print(&inst)?)),
            Item::Bytes(bytes) => out.push_str(&format!("\t.byte\t{}\n", bytes.len())),
            Item::Table(labels) => labels.iter().for_each(|label| out.push_str(&format!("\t.long\t{label}\n"))),
            Item::Loc(row) if debug => {
                // A row on the line the one before it was on does not start a
                // statement.
                let flags = format!(
                    "{}{}{}",
                    if row.prologue_end { " prologue_end" } else { "" },
                    if row.epilogue_begin { " epilogue_begin" } else { "" },
                    if row.place.line == 0 || (previous == Some(row.place.line) && !row.block_top) {
                        " is_stmt 0"
                    } else {
                        ""
                    },
                );
                previous = (row.place.line != 0).then_some(row.place.line);
                out.push_str(&format!("\t.loc\t0 {} {}{flags}\n", row.place.line, row.place.column));
            }
            Item::Loc(_) => {}
        }
    }
    Ok(out)
}

thread_local! {
    static MID_ENDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times this thread has run the mid-end over a module, for a test
/// that a driver runs it once per module and not once per function.
pub fn mid_end_runs() -> usize {
    MID_ENDS.with(std::cell::Cell::get)
}

/// `module` through the IR pass `llc` runs before selection that matters here,
/// loop strength reduction, priced by [`M68k`]. (`llc` runs no `opt` pipeline;
/// the bench's reference does not either.)
pub fn mid_end(module: &Module) -> Result<Module, String> {
    mid_end_for(module, Cpu::default())
}

/// [`mid_end`] for `cpu`.
pub fn mid_end_for(
    module: &Module,
    cpu: Cpu,
) -> Result<Module, String> {
    mid_end_described_by(module, &M68k::for_cpu(cpu), Some(Rc::new(M68kMachine { cpu })))
}

/// [`mid_end_for`], narrowing multiplies where `narrow_multiplies`.
pub fn mid_end_for_narrowing(
    module: &Module,
    cpu: Cpu,
    narrow_multiplies: bool,
) -> Result<Module, String> {
    mid_end_narrowing(module, &M68k::for_cpu(cpu), Some(Rc::new(M68kMachine { cpu })), narrow_multiplies)
}

/// [`mid_end`] with another target description: what a test compares the
/// m68k one with.
pub fn mid_end_described_by(
    module: &Module,
    tti: &dyn llrm_mir::tti::Tti,
    machine: Option<Rc<dyn llrm_mir::target::Machine>>,
) -> Result<Module, String> {
    mid_end_narrowing(module, tti, machine, false)
}

/// [`mid_end_described_by`], narrowing the multiplies the range proves narrow
/// (`narrow_multiplies`: -O2 and -O3, not for size).
pub fn mid_end_narrowing(
    module: &Module,
    tti: &dyn llrm_mir::tti::Tti,
    machine: Option<Rc<dyn llrm_mir::target::Machine>>,
    narrow_multiplies: bool,
) -> Result<Module, String> {
    MID_ENDS.with(|runs| runs.set(runs.get() + 1));
    use llrm_mir::datalayout::DataLayout;
    use llrm_mir::passes::{Declared, Outer};
    let mut module = module.clone();
    let layout = match module.datalayout.as_deref() {
        Some(text) => DataLayout::parse(text)?,
        None => DataLayout::default(),
    };
    let names: Vec<String> = module
        .functions()
        .filter(|(_, _, function)| !function.is_declaration())
        .filter_map(|(_, global, _)| global.name.as_deref().map(str::to_owned))
        .collect();
    let outer = Outer::of(&module, machine);
    let mut declared = Declared::of(&module);
    for name in names {
        let (context, function) = module.function_mut(&name).ok_or("a function that vanished")?;
        if llrm_transforms::exitne::enabled() && tti.prefers_exit_ne() {
            llrm_transforms::exitne::strengthened(context, &layout, function, &outer);
        }
        llrm_transforms::lsr_pass::reduced_with(context, &layout, function, &mut declared, &outer, tti, false);
        // after it: it folds `zext (trunc iv)` back into the counter
        let for_size = has(&function.attrs, "optsize") || has(&function.attrs, "minsize");
        if let (true, false, Some(limit)) = (narrow_multiplies, for_size, tti.range_narrowing_width()) {
            llrm_transforms::narrowrange::multiplies_narrowed(context, &layout, function, &outer, limit);
        }
        // the ids the pass handed out for declarations name nothing until they
        // are placed
        declared.place(&mut module)?;
    }
    Ok(module)
}
