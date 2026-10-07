//! The one compiler past the frontends, as clang's CodeGen and llc: a HIR
//! program emitted as MIR, or a lifter's MIR modules, linked against the
//! runtime and optimized by the pipeline for the machine. It reads what
//! the program states and never asks which frontend made it.

pub mod basic;
mod data;
pub mod flags;

use std::collections::HashMap;
use std::path::PathBuf;

use llrm_mir::program::Program;
use llrm_mir::{GlobalId, Module};

use crate::abi::machine::Machine;
use crate::abi::qb::HirAbi;
use crate::backend::{assemble, executed};
use crate::backend::cpu::{self, Profile, ProfileOrName};
use crate::backend::masm;
use crate::backend::target::Segments;
use crate::hir::model;
use crate::model::ir::{Operation, Semantics};
use crate::model::lir;
use data::Placed;
use llrm_support::debug::timed;

/// What a compile is for: the machine, whose CPU prices the choices, the
/// passes that run, and where the pipeline writes each stage.
#[derive(Clone)]
pub struct Options {
    pub machine: Machine,
    pub pipeline: llrm_transforms::pipeline::Options,
    pub dump: Option<PathBuf>,
    /// `-fstack-usage`: print each procedure's frame and what it can reach.
    pub stack_usage: bool,
    /// `-Wstack-usage=N`: warn of each entry that can reach more than N bytes.
    pub stack_limit: Option<i64>,
    /// The target's instruction selector, which `llrm-driver` binds to it.
    pub selection: &'static crate::backend::isel::Compiled,
    /// The target `selection` is for.
    pub arch: std::rc::Rc<dyn llrm_target::Target>,
    /// The object format the symbols are spelled for: `omf`, `elf` or `macho`, which the target's conventions decorate.
    pub object_format: &'static str,
}

impl Options {
    /// For `machine` on the target `arch` with its selector, at -O2, the stages
    /// written where `LLRM_MIR_STAGES` names.
    pub fn new(machine: Machine, arch: std::rc::Rc<dyn llrm_target::Target>, selection: &'static crate::backend::isel::Compiled) -> Self {
        Self { machine, pipeline: Default::default(), dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into), stack_usage: false, stack_limit: None, selection, arch, object_format: "omf" }
    }

    /// For 16-bit x86, which the tests of this crate are written for.
    #[cfg(test)]
    pub fn m16(machine: Machine) -> Self {
        Self::new(machine, std::rc::Rc::new(llrm_x86_m16::M16), crate::backend::isel::m16())
    }

    pub fn cpu(&self) -> Result<&'static Profile, String> {
        cpu::tuned_for(&*self.arch, &self.machine.cpu, self.pipeline.prefers_size())
    }
}

/// `program` compiled for the machine: emitted, optimized, then each module
/// selected and assembled, its code in `<MODULE>_TEXT`, each function linked
/// by its symbol and its data where the frontend put it. Each module's
/// listing and executed costs go beside the stages.
pub fn compiled(program: &model::Program, options: &Options) -> Result<Vec<masm::Module>, String> {
    let (mut mir, data) = emitted(program, options)?;
    let placed: Vec<Placed> = timed("data placement", || mir.modules.iter().zip(&program.modules).zip(&data).map(|((module, hir), data)| Placed::of(module, hir, data)).collect());
    optimized(&mut mir, options)?;
    let abi = HirAbi::of(program)?;
    let segments = Segments::of(&options.machine);
    let mut out = Vec::new();
    for ((module, hir), placed) in mir.modules.iter().zip(&program.modules).zip(&placed) {
        let mut assembled = timed("assemble", || assemble::assembled_by(module, &abi, &format!("{}_TEXT", hir.name.to_uppercase()), ProfileOrName::Profile(options.cpu()?), &segments, options.selection, &*options.arch))?;
        timed("data layout", || placed.lay_out(&mut assembled, module, mir.segments.data_space, program.constant_segment.as_deref(), options.machine.far_bss, options.arch.layout().segment_bytes()))?;
        if let Some(directory) = &options.dump {
            let suffix = if program.modules.len() > 1 { format!("-{}", hir.name) } else { String::new() };
            let written = |name: &str, text: String| std::fs::write(directory.join(format!("{name}{suffix}")), text).map_err(|error| error.to_string());
            written("listing.asm", masm::text(&assembled).map_err(|error| error.to_string())?)?;
            let cpu = options.cpu()?;
            written("cost", assembled.procedures.iter().map(|one| executed::summary(&one.body, cpu) + "\n").collect())?;
        }
        out.push(assembled);
    }
    if options.stack_usage || options.stack_limit.is_some() {
        let usage = crate::backend::stackusage::Usage::of(&out, &*options.arch);
        if options.stack_usage {
            eprint!("{}", usage.report());
        }
        for warning in options.stack_limit.into_iter().flat_map(|limit| usage.warnings(limit)) {
            eprintln!("{warning}");
        }
    }
    Ok(out)
}

/// What the spill model (`llrm_transforms::spill`) forecasts for each function the pipeline
/// hands to instruction selection: the `spillmodel` channel, to set beside the spills the
/// allocator makes (`cost` channel, `executed`).
fn spill_model(program: &Program) {
    use llrm_transforms::{profit, spill};
    for module in &program.modules {
        let Some(text) = module.datalayout.as_deref() else { continue };
        let Ok(layout) = llrm_mir::datalayout::DataLayout::parse(text) else { continue };
        let outer = llrm_mir::passes::Outer::of(module, Some(program.target.clone()));
        let room = spill::Room::of(&outer);
        let costs = program.target.costs();
        for global in &module.globals {
            let Some(function) = global.function().filter(|one| !one.is_declaration()) else { continue };
            let mut analyses = llrm_mir::passes::Analyses::new(std::rc::Rc::new(outer.clone()));
            let registers = analyses.get::<llrm_analysis::manager::Registers>(&module.context, &layout, function);
            let unit = llrm_analysis::memory::Unit::of(module, &layout, function).with_spaces(program.target.spaces()).with_registers(&registers);
            let trips = profit::proven_trips(&unit, &registers);
            let Some(frequency) = profit::_frequencies(&module.context, &module.metadata, &module.globals, function, Some(&trips)) else { continue };
            let across = |inst| spill::kept_across(&outer, &module.context, function, inst);
            if let Some(forecast) = profit::spill_forecast(&module.context, &layout, function, &costs, room, &across, &frequency) {
                llrm_support::debug!("spillmodel", "{} peak {} spilled {} price {}", global.name.as_deref().unwrap_or("?"), forecast.peak, forecast.spilled.len(), forecast.cost);
                if let Some(name) = global.name.as_deref() {
                    let per_entry = forecast.cost as f64 / profit::UNIT as f64;
                    crate::backend::executed::predict(name, crate::backend::executed::Predicted { peak: forecast.peak, spilled: forecast.spilled.len(), price: per_entry, load: costs.load, store: costs.store });
                }
            }
        }
    }
}

/// `program` as MIR, a module per HIR module, linked against the runtime
/// its promises describe; and each module's data objects' globals, by the
/// objects' ids.
pub fn emitted(program: &model::Program, options: &Options) -> Result<(Program, Vec<HashMap<i64, GlobalId>>), String> {
    // Whichever frontend made it, a program is checked before it is lowered.
    crate::support::debug::timed("hir verify", || llrm_hir::verify::verify(program)).map_err(|why| why.0)?;
    let emitted = timed("hir to mir", || llrm_hir::mir::emit(program, &options.arch.layout()));
    if let Some((name, why)) = emitted.iter().find_map(|one| one.refused.first()) {
        return Err(format!("@{name}: {why}"));
    }
    let runtime = timed("mir runtime", || crate::hir::mir::runtime(&emitted.iter().zip(&program.modules).collect::<Vec<_>>(), &program.promises))?;
    let (modules, data) = emitted.into_iter().map(|one| (one.module, one.data)).unzip();
    let target = std::rc::Rc::new(crate::abi::qb::LoweredTarget::of(options.cpu()?, crate::abi::qb::HirAbi::of(program)?));
    let mut linked = timed("mir link", || linked(modules, runtime, target))?;
    linked.exports.entries = program.entries.iter().cloned().collect();
    Ok((linked, data))
}

/// `modules` as one program for `target`, linked against `runtime`, a
/// module of declarations alone; each verified.
pub fn linked(modules: Vec<Module>, runtime: Module, target: std::rc::Rc<dyn llrm_mir::target::Machine>) -> Result<Program, String> {
    let program = Program::new(modules, target)?.with_runtime(runtime)?;
    timed("mir verify frontend", || verified(&program, "the frontend"))?;
    Ok(program)
}

/// `program` through the pipeline, then each module prepared for
/// instruction selection: a landing pad made one the runtime enters. Each
/// module verified after.
pub fn optimized(program: &mut Program, options: &Options) -> Result<(), String> {
    let applied = llrm_transforms::pipeline::Applied { options: options.pipeline.clone(), dump: options.dump.clone(), ..Default::default() };
    timed("mir pipeline", || llrm_transforms::pipeline::applied(program, &applied))?;
    timed("mir assumptions", || program.modules.iter_mut().for_each(llrm_transforms::dead::assumptions_dropped));
    timed("mir ehprepare", || program.modules.iter_mut().try_for_each(crate::backend::ehprepare::prepared))?;
    timed("mir selects", || program.modules.iter_mut().try_for_each(crate::backend::selects::lowered))?;
    if llrm_support::debug::enabled("spillmodel") || llrm_support::debug::enabled("pressure") {
        timed("mir spill model", || spill_model(program));
    }
    timed("mir verify pipeline", || verified(program, "the pipeline"))
}

/// Refuses `program` where a module does not verify, `stage` having made it.
fn verified(program: &Program, stage: &str) -> Result<(), String> {
    match program.modules.iter().find_map(|module| llrm_mir::verify::verify(module).into_iter().next()) {
        Some(first) => Err(format!("{stage} left invalid MIR: {first}")),
        None => Ok(()),
    }
}

/// The data no frontend lays out, which emission and the pipeline made --
/// ON ERROR's ERL table and the ERR its landing keeps -- each defined
/// variable `laid` does not claim, for a DGROUP segment.
pub fn added_data(module: &Module, laid: &dyn Fn(GlobalId) -> bool, names: &crate::support::hash::IndexMap<(crate::model::ir::Space, i64), String>) -> Result<Vec<masm::Datum>, String> {
    let added = module.globals.iter().enumerate().map(|(at, _)| GlobalId(at as u32)).filter(|&id| {
        matches!(&module.global(id).kind, llrm_mir::GlobalKind::Variable(variable) if variable.initializer.is_some()) && !laid(id)
    });
    added.map(|id| crate::backend::globals::datums(module, id, names)).collect::<Result<Vec<_>, _>>().map(|all| all.concat())
}

/// Whether a driver frames `function`: a naked one, as the landing stub,
/// runs on the frame the runtime made.
pub fn framed(module: &Module, function: GlobalId) -> bool {
    !module.global(function).function().is_some_and(|one| one.attrs.iter().any(|attr| matches!(attr, llrm_mir::Attribute::Flag(flag) if flag == "naked")))
}

/// The statement-table row RESUME NEXT reaches a landing pad by: the pad's
/// block `landing`, in the procedure assembled `number`th.
pub fn landing_row(number: usize, landing: i64) -> (i64, i64, String, i64) {
    (number as i64, 0, masm::label(number, landing), 0)
}

/// The statement-table row at the procedure assembled `number`th's first
/// block `entry`, at line 0: the runtime reports an error's line from the
/// last row before it, and no numbered line precedes this one's code.
pub fn entry_row(number: usize, entry: i64) -> (i64, i64, String, i64) {
    (number as i64, -1, masm::label(number, entry), 0)
}

/// OF_STA's table: each row a statement's offset and BASIC line, ended by
/// a zero word, as a procedure of inline data.
pub fn statement_table(rows: &[(i64, i64, String, i64)], registers: llrm_target::FrameRegisters) -> masm::Procedure {
    let mut code: Vec<masm::InlinePart> = Vec::new();
    for (_procedure, _order, label, line) in rows {
        code.push(masm::InlinePart::Fixup("offset".into(), label.clone(), 0));
        code.push(masm::InlinePart::Bytes((*line as u16).to_le_bytes().to_vec()));
    }
    code.push(masm::InlinePart::Bytes(vec![0, 0]));
    let what = Semantics { name: Some("statement-table".to_owned()), ..Semantics::new(Operation::Call) };
    let instruction = lir::Insn::new(1, None, Some(what), vec![], vec![]);
    let body = lir::LirBody::new("$QB$STAT", 1, vec![lir::LirBlock::new(1, vec![std::sync::Arc::new(instruction)])], Default::default(), Default::default());
    masm::Procedure {
        name: "$QB$STAT".into(),
        public: false,
        far: false,
        body,
        reserve: 0,
        callees: crate::support::hash::IndexMap::from_iter([(1, masm::Callee { name: "$statement-table".into(), far: false, pops: 0, code })]),
        interrupt: None,
        size: false,
        entry: 0,
        stack_check: None,
        registers,
    }
}

#[cfg(test)]
mod lifetimes_tests;
