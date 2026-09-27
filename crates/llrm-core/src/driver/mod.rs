//! The one compiler past the frontends, as clang's CodeGen and llc: a HIR
//! program emitted as MIR, or a lifter's MIR modules, linked against the
//! runtime and optimized by the pipeline for the machine. It reads what
//! the program states and never asks which frontend made it.

pub mod basic;
mod data;

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
use data::Placed;

/// What a compile is for: the machine, whose CPU prices the choices, and
/// where the pipeline writes each stage.
pub struct Options {
    pub machine: Machine,
    pub dump: Option<PathBuf>,
}

impl Options {
    /// For `machine`, the stages written where `LLRM_MIR_STAGES` names.
    pub fn of(machine: Machine) -> Self {
        Self { machine, dump: std::env::var_os("LLRM_MIR_STAGES").map(Into::into) }
    }

    pub fn cpu(&self) -> Result<&'static Profile, String> {
        cpu::named(&self.machine.cpu)
    }
}

/// `program` compiled for the machine: emitted, optimized, then each module
/// selected and assembled, its code in `<MODULE>_TEXT`, each function linked
/// by its symbol and its data where the frontend put it. Each module's
/// listing and executed costs go beside the stages.
pub fn compiled(program: &model::Program, options: &Options) -> Result<Vec<masm::Module>, String> {
    let (mut mir, data) = emitted(program, options)?;
    let placed: Vec<Placed> = mir.modules.iter().zip(&program.modules).zip(&data).map(|((module, hir), data)| Placed::of(module, hir, data)).collect();
    optimized(&mut mir, options)?;
    let abi = HirAbi::of(program)?;
    let segments = Segments::of(&options.machine);
    let mut out = Vec::new();
    for ((module, hir), placed) in mir.modules.iter().zip(&program.modules).zip(&placed) {
        let mut assembled = assemble::assembled(module, &abi, &format!("{}_TEXT", hir.name.to_uppercase()), ProfileOrName::Profile(options.cpu()?), &segments)?;
        placed.lay_out(&mut assembled, module, mir.segments.data_space, program.constant_segment.as_deref())?;
        if let Some(directory) = &options.dump {
            let suffix = if program.modules.len() > 1 { format!("-{}", hir.name) } else { String::new() };
            let written = |name: &str, text: String| std::fs::write(directory.join(format!("{name}{suffix}")), text).map_err(|error| error.to_string());
            written("listing.asm", masm::text(&assembled).map_err(|error| error.to_string())?)?;
            written("cost", assembled.procedures.iter().map(|one| executed::summary(&one.body) + "\n").collect())?;
        }
        out.push(assembled);
    }
    Ok(out)
}

/// `program` as MIR, a module per HIR module, linked against the runtime
/// its promises describe; and each module's data objects' globals, by the
/// objects' ids.
pub fn emitted(program: &model::Program, options: &Options) -> Result<(Program, Vec<HashMap<i64, GlobalId>>), String> {
    let emitted = crate::hir::mir::emit(program);
    if let Some((name, why)) = emitted.iter().find_map(|one| one.refused.first()) {
        return Err(format!("@{name}: {why}"));
    }
    let runtime = crate::hir::mir::runtime(&emitted.iter().zip(&program.modules).collect::<Vec<_>>(), &program.promises)?;
    let (modules, data) = emitted.into_iter().map(|one| (one.module, one.data)).unzip();
    let mut linked = linked(modules, runtime, options)?;
    linked.exports.entries = program.entries.iter().cloned().collect();
    Ok((linked, data))
}

/// `modules` as one program for the machine, linked against `runtime`, a
/// module of declarations alone; each verified.
pub fn linked(modules: Vec<Module>, runtime: Module, options: &Options) -> Result<Program, String> {
    let program = Program::new(modules, options.cpu()?.target())?.with_runtime(runtime)?;
    verified(&program, "the frontend")?;
    Ok(program)
}

/// `program` through the pipeline, each module verified after.
pub fn optimized(program: &mut Program, options: &Options) -> Result<(), String> {
    let applied = llrm_transforms::pipeline::Applied { dump: options.dump.clone(), ..Default::default() };
    llrm_transforms::pipeline::applied(program, &applied)?;
    verified(program, "the pipeline")
}

/// Refuses `program` where a module does not verify, `stage` having made it.
fn verified(program: &Program, stage: &str) -> Result<(), String> {
    match program.modules.iter().find_map(|module| llrm_mir::verify::verify(module).into_iter().next()) {
        Some(first) => Err(format!("{stage} left invalid MIR: {first}")),
        None => Ok(()),
    }
}
