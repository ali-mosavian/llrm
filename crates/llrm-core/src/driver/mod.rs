//! The one compiler past the frontends, as clang's CodeGen and llc: a HIR
//! program emitted as MIR, or a lifter's MIR modules, linked against the
//! runtime and optimized by the pipeline for the machine. It reads what
//! the program states and never asks which frontend made it.

pub mod basic;

use std::collections::HashMap;
use std::path::PathBuf;

use llrm_mir::program::Program;
use llrm_mir::{GlobalId, Module};

use crate::abi::machine::Machine;
use crate::abi::qb::HirAbi;
use crate::backend::assemble;
use crate::backend::cpu::{self, Profile, ProfileOrName};
use crate::backend::masm;
use crate::backend::target::Segments;
use crate::hir::model;

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
/// selected and assembled, its code in `<MODULE>_TEXT` and each function
/// linked by its symbol. Each module's listing goes beside the stages.
pub fn compiled(program: &model::Program, options: &Options) -> Result<Vec<masm::Module>, String> {
    let (mut mir, _) = emitted(program, options)?;
    optimized(&mut mir, options)?;
    let functions = program.modules.iter().flat_map(|module| &module.functions);
    let abi = HirAbi { runtime: program.runtime, objects: functions.filter_map(|one| Some((one.name.clone(), one.symbol.clone()?))).collect() };
    let segments = Segments::of(&options.machine);
    let mut out = Vec::new();
    for (module, hir) in mir.modules.iter().zip(&program.modules) {
        let assembled = assemble::assembled(module, &abi, &format!("{}_TEXT", hir.name.to_uppercase()), ProfileOrName::Profile(options.cpu()?), &segments)?;
        if let Some(directory) = &options.dump {
            let name = if program.modules.len() > 1 { format!("listing-{}.asm", hir.name) } else { "listing.asm".to_owned() };
            std::fs::write(directory.join(name), masm::text(&assembled).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
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
/// module of declarations alone.
pub fn linked(modules: Vec<Module>, runtime: Module, options: &Options) -> Result<Program, String> {
    Program::new(modules, options.cpu()?.target())?.with_runtime(runtime)
}

/// `program` through the pipeline.
pub fn optimized(program: &mut Program, options: &Options) -> Result<(), String> {
    let applied = llrm_transforms::pipeline::Applied { dump: options.dump.clone(), ..Default::default() };
    llrm_transforms::pipeline::applied(program, &applied)
}
