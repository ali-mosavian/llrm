//! The one compiler past the frontends, as clang's CodeGen and llc: a HIR
//! program emitted as MIR, or a lifter's MIR modules, linked against the
//! runtime and optimized by the pipeline for the machine. It reads what
//! the program states and never asks which frontend made it.

use std::collections::HashMap;
use std::path::PathBuf;

use llrm_mir::program::Program;
use llrm_mir::{GlobalId, Module};

use crate::abi::machine::Machine;
use crate::backend::cpu::{self, Profile};

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

/// `program` as MIR, a module per HIR module, linked against the runtime
/// its promises describe; and each module's data objects' globals, by the
/// objects' ids.
pub fn emitted(program: &crate::hir::model::Program, options: &Options) -> Result<(Program, Vec<HashMap<i64, GlobalId>>), String> {
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
