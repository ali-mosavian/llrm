//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use std::rc::Rc;

use llrm_mir::memory::has;
use llrm_mir::module::Module;

use crate::dagisel::function::select_function_with;
use crate::dagisel::hooks::Cpu;
use crate::expand::{Line, finish_with};
use crate::flags::preserve_flags;
use crate::machine::{M68k, M68kMachine};
use crate::mc;
use crate::mfunc::{emit, expand_cmov};
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
    /// The source place of the instructions that follow (`.loc`): printed only
    /// for `-g`.
    Loc(crate::expand::Row),
}

/// The pin's code model (`-mcmodel`), which decides how a global is reached
/// (`M68kSubtarget::classifyGlobalReference`, without PIC): small, PC-relative
/// with a 16-bit displacement; medium, absolute on the 68000 and 68010 and
/// PC-relative from the 68020; large, absolute. The assembler relaxes only
/// branches, so data farther than 32 KiB from the code needs medium on the
/// 68000 or large.
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

    /// Whether a global's address is an absolute one on `cpu`.
    pub fn absolute_data(
        self,
        cpu: Cpu,
    ) -> bool {
        match self {
            CodeModel::Small => false,
            CodeModel::Medium => !cpu.at_least(Cpu::M68020),
            CodeModel::Large => true,
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
    /// A load and the step of its pointer in another block are one `(An)+`
    /// ([`crate::loopinc`]).
    pub post_inc_across_blocks: bool,
    /// A small conditional block is copied into the predecessors that jump to
    /// it, inside a loop ([`llrm_spike_mf::taildup`]); not in a size build.
    pub tail_duplicate: bool,
    /// A load of an incoming stack argument is made again where it is read
    /// ([`crate::regalloc::Registers`], M68K-ARGREMAT).
    pub rematerialize_arguments: bool,
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
        strict: false,
        source_order: false,
        indexed: false,
        push_frame: false,
        push_arguments: false,
        short: false,
        hoist_flag_setters: false,
        reuse_shift_flags: false,
        post_inc_across_blocks: false,
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
            strict: false,
            source_order: false,
            indexed: true,
            push_frame: true,
            push_arguments: true,
            short: true,
            hoist_flag_setters: true,
            reuse_shift_flags: true,
            post_inc_across_blocks: true,
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
pub fn items_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<Vec<Item>, String> {
    let optimized;
    let module = if options.mid_end {
        optimized = mid_end_described_by(module, &M68k::for_cpu(cpu), Some(Rc::new(M68kMachine { cpu })))?;
        &optimized
    } else {
        module
    };
    let mut options = Options { mid_end: false, global_copies: options.global_copies, ..options };
    let (mut kept, touched) = items_once(module, name, cpu, options)?;
    if !touched.any() {
        return Ok(kept);
    }
    let wants_size = options.size
        || module
            .named(name)
            .and_then(|id| module.global(id).function())
            .is_some_and(|one| has(&one.attrs, "optsize") || has(&one.attrs, "minsize"));
    let price = crate::timing::Price::of(wants_size, options.strict);
    if touched.sext_adds {
        let without = Options { sext_adds: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.class_split {
        let without = Options { class_split: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.return_dup {
        let without = Options { return_dup: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.global_copies {
        // the default stands unless the other join order is cheaper
        let other = Options { global_copies: false, ..options };
        let (items, _) = items_once(module, name, cpu, other)?;
        if crate::trial::keeps(&items, &kept, cpu, price) {
            kept = items;
            options = other;
        }
    }
    // last: the two builds compared are final, the other choices made
    if touched.carry_chain {
        let without = Options { carry_chain: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
            options = without;
        }
    }
    if touched.compare_destinations {
        let without = Options { compare_destinations: false, ..options };
        let (plain, _) = items_once(module, name, cpu, without)?;
        if !crate::trial::keeps(&kept, &plain, cpu, price) {
            kept = plain;
        }
    }
    Ok(kept)
}

/// Whether `function` carries through the extend flag: an `addx`, `subx` or
/// `negx` (a subtract from a constant zero high half is `negx`).
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
    sext_adds: bool,
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
}

impl Touched {
    fn any(self) -> bool {
        self.sext_adds
            || self.return_dup
            || self.class_split
            || self.compare_destinations
            || self.carry_chain
            || self.global_copies
    }
}

/// One build of the function, and the optional rewrites that changed it.
fn items_once(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<(Vec<Item>, Touched), String> {
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let number =
        module.functions().filter(|(_, _, one)| !one.is_declaration()).position(|(one, _, _)| one == id).unwrap_or(0);
    let wants_size = options.size
        || module.global(id).function().is_some_and(|one| has(&one.attrs, "optsize") || has(&one.attrs, "minsize"));
    let blocks = select_function_with(
        module,
        name,
        cpu,
        options.source_order,
        options.indexed,
        options.short,
        options.code_model.absolute_data(cpu),
        wants_size,
        options.carry_chain,
    )?;
    let mut function = emit(name, &blocks)?;
    let mut touched = Touched::default();
    touched.carry_chain = options.carry_chain && uses_extend(&function);
    function.returns_twice =
        module.global(id).function().is_some_and(|one| llrm_mir::memory::calls_returns_twice(module, one));
    if options.short {
        crate::cmpelim::eliminate_tests(&mut function, options.hoist_flag_setters);
        if options.reuse_shift_flags {
            crate::cmpshift::reuse_shift_flags(&mut function);
        }
        crate::shiftadd::shifts_to_adds(&mut function);
        crate::negadd::negative_adds_to_subq(&mut function);
        crate::addrarith::address_adds(&mut function, cpu, wants_size);
        crate::foldload::fold_loads(&mut function);
        crate::divrem::remainders_from_quotients(&mut function);
        if wants_size {
            crate::foldarg::fold_argument_loads(&mut function);
        }
        crate::addrroles::address_roles(&mut function);
        if options.compare_destinations {
            touched.compare_destinations = crate::addrcmp::widen_compare_destinations(&mut function) > 0;
        }
        if options.sext_adds {
            touched.sext_adds = crate::sextacc::sign_extended_words(
                &mut function,
                cpu,
                crate::timing::Price::of(wants_size, options.strict),
            ) > 0;
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
    machine_sink(&mut function);
    crate::regalloc::split_flag_edges(&mut function);
    eliminate_phis_with(&mut function, false);
    fold_reserved_copies(&mut function);
    if options.short {
        crate::commutative::mark_commutative(&mut function, wants_size);
    }
    commute_tied(&mut function);
    two_address(&mut function);
    // spill slots are numbered after the static allocas
    let first_slot = function.locals.iter().map(|local| local.0 + 1).max().unwrap_or(0);
    let (assignment, slots) = if std::env::var_os("LLRM_M68K_SPIKE").is_some() {
        crate::regalloc::allocate_spilling_by_frequency(&mut function, first_slot)?
    } else {
        crate::regalloc::allocate_greedy(
            &mut function,
            wants_size,
            options.indexed,
            options.rematerialize_arguments,
            options.global_copies,
            options.loop_carried_first,
        )?
    };
    let slots: Vec<i32> = slots.into_iter().map(|(slot, _)| slot).collect();
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
    llrm_spike_mf::spill::remove_dead_slot_stores::<crate::regalloc::M68kRegisters>(
        &mut function,
        &assignment,
        &crate::REGISTER_INFO,
        &slots,
    );
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
    branch_fold(&mut function, &assignment);
    if options.tail_duplicate && !wants_size {
        llrm_spike_mf::taildup::tail_duplicate::<_, crate::regalloc::M68kRegisters>(&mut function, 2);
        if options.return_dup {
            touched.return_dup = llrm_spike_mf::taildup::duplicate_returns(&mut function, 2);
        }
    }
    crate::regalloc::place_blocks(&mut function, &edge_odds(module, name)?, wants_size, options.loop_top_by_jumps);
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
    Ok((out, touched))
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

/// [`mid_end`] with another target description: what a test compares the
/// m68k one with.
pub fn mid_end_described_by(
    module: &Module,
    tti: &dyn llrm_mir::tti::Tti,
    machine: Option<Rc<dyn llrm_mir::target::Machine>>,
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
        // the ids the pass handed out for declarations name nothing until they
        // are placed
        declared.place(&mut module)?;
    }
    Ok(module)
}
