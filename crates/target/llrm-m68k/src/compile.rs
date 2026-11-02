//! A function from IR to assembly text: the shared DAG stages, then the
//! m68k machine function, allocation, expansion and printing.

use std::rc::Rc;

use llrm_mir::memory::has;
use llrm_mir::module::Module;

use crate::dagisel::function::select_function_with;
use crate::dagisel::hooks::Cpu;
use crate::expand::{Line, finish};
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
}

/// What the compiler may do beyond the pin's output.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Run the shared IR mid-end (`-O2`) first, as `llc -O2` is given
    /// optimized IR by `opt`.
    pub mid_end: bool,
    /// Size is wanted over clocks in every function (`-Os`; the x86 path's
    /// `Cpu::size`). A function marked `optsize` or `minsize` wants it too.
    pub size: bool,
    /// Schedule in source order (`-pre-RA-sched=source`) rather than with the
    /// ILP list scheduler `llc` defaults to.
    pub source_order: bool,
    /// Form `(An)+` and `-(An)` accesses, which gcc does and the pin does not.
    pub indexed: bool,
    /// Push and pop the saved registers where that is no larger and no
    /// slower than the pin's `suba.l` and `movem` into slots.
    pub push_frame: bool,
    /// `addq` and `subq` for an immediate of 1 to 8, which the pin has no
    /// instruction for.
    pub short: bool,
}

impl Options {
    /// The pin's output, instruction for instruction: what `llc` and `llvm-mc`
    /// can be compared with.
    pub const PINNED: Options =
        Options { mid_end: false, size: false, source_order: false, indexed: false, push_frame: false, short: false };
}

impl Default for Options {
    fn default() -> Options {
        Options { mid_end: true, size: false, source_order: false, indexed: true, push_frame: true, short: true }
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
pub fn items_with(
    module: &Module,
    name: &str,
    cpu: Cpu,
    options: Options,
) -> Result<Vec<Item>, String> {
    let optimized;
    let module = if options.mid_end {
        optimized = mid_end_described_by(module, &M68k::default(), Some(Rc::new(M68kMachine)))?;
        &optimized
    } else {
        module
    };
    let id = module.named(name).ok_or_else(|| format!("no function @{name}"))?;
    let number =
        module.functions().filter(|(_, _, one)| !one.is_declaration()).position(|(one, _, _)| one == id).unwrap_or(0);
    let wants_size = options.size
        || module.global(id).function().is_some_and(|one| has(&one.attrs, "optsize") || has(&one.attrs, "minsize"));
    let blocks = select_function_with(module, name, cpu, options.source_order, options.indexed, options.short)?;
    let mut function = emit(name, &blocks)?;
    if options.short {
        crate::cmpelim::eliminate_tests(&mut function);
    }
    expand_cmov(&mut function)?;
    machine_sink(&mut function);
    crate::regalloc::split_flag_edges(&mut function);
    eliminate_phis_with(&mut function, false);
    fold_reserved_copies(&mut function);
    commute_tied(&mut function);
    two_address(&mut function);
    // spill slots are numbered after the static allocas
    let first_slot = function.locals.iter().map(|local| local.0 + 1).max().unwrap_or(0);
    let (assignment, slots) = if std::env::var_os("LLRM_M68K_SPIKE").is_some() {
        crate::regalloc::allocate_spilling_by_frequency(&mut function, first_slot)?
    } else {
        crate::regalloc::allocate_greedy(&mut function, wants_size)?
    };
    let slots: Vec<i32> = slots.into_iter().map(|(slot, _)| slot).collect();
    remove_identity_copies(&mut function, &assignment);
    remove_redundant_copies(&mut function, &assignment);
    preserve_flags(&mut function, &assignment)?;
    branch_fold(&mut function, &assignment);
    crate::regalloc::place_blocks(&mut function, &edge_odds(module, name)?, wants_size);
    let mut out = vec![Item::Label(name.to_owned())];
    for line in finish(&function, &assignment, number, &slots, cpu, options.push_frame)? {
        out.push(match line {
            Line::Label(at) => Item::Label(format!(".LBB{number}_{at}")),
            Line::Inst(inst) => Item::Inst(inst),
        });
    }
    Ok(out)
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
    let mut out = String::new();
    for item in items_with(module, name, cpu, options)? {
        match item {
            Item::Label(label) => out.push_str(&format!("{label}:\n")),
            Item::Inst(inst) => out.push_str(&format!("\t{}\n", mc::print(&inst)?)),
            Item::Bytes(bytes) => out.push_str(&format!("\t.byte\t{}\n", bytes.len())),
        }
    }
    Ok(out)
}

/// `module` through the IR pass `llc` runs before selection that matters here,
/// loop strength reduction, priced by [`M68k`]. (`llc` runs no `opt` pipeline;
/// the bench's reference does not either.)
pub fn mid_end(module: &Module) -> Result<Module, String> {
    mid_end_described_by(module, &M68k::default(), Some(Rc::new(M68kMachine)))
}

/// [`mid_end`] with another target description: what a test compares the
/// m68k one with.
pub fn mid_end_described_by(
    module: &Module,
    tti: &dyn llrm_mir::tti::Tti,
    machine: Option<Rc<dyn llrm_mir::target::Machine>>,
) -> Result<Module, String> {
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
