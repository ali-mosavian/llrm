//! Port of `qbopt/frontend/modern/compile.py`: common HIR from the modern
//! frontend to a fresh 16-bit OMF object.

use std::collections::BTreeSet;
use std::path::Path;

use llrm_core::backend::{masm, objbuild};
use llrm_core::hir::model;
use llrm_core::support::pyrepr;

// DOS C symbols carry one leading underscore. Runtime builtins already
// use their compact, mangled ABI names (for example `N$PS`), and an exported
// function is named by its ABI's symbol.
fn object_name(function: &model::Function) -> String {
    if function.linkage == model::FunctionLinkage::External || function.name.starts_with("__") {
        function.name.clone()
    } else {
        format!("_{}", function.name)
    }
}

/// Compile one Nib module: the HIR emitted as MIR, optimized, then selected
/// and assembled whole.
pub fn assembled(
    program: &model::Program,
    entry: &str,
    options: &llrm_core::driver::Options,
    os: &crate::Os,
) -> Result<masm::Module, String> {
    assembled_for(program, Some(entry), options, os)
}

/// `assembled` for a library cut to what some objects name (`keep_exports`): nothing may be left of it that calls an
/// entry, and a cut to nothing is still a library, one with no export.
pub fn assembled_library(
    program: &model::Program,
    options: &llrm_core::driver::Options,
    os: &crate::Os,
) -> Result<masm::Module, String> {
    assembled_for(program, None, options, os)
}

fn assembled_for(
    program: &model::Program,
    entry: Option<&str>,
    options: &llrm_core::driver::Options,
    os: &crate::Os,
) -> Result<masm::Module, String> {
    if program.modules.len() != 1 {
        return Err("native Nib compilation currently accepts one module".to_owned());
    }
    // Each function links by its object name; the entry, `_main`, is public
    // for the runtime to call, and a library has none. The pipeline's
    // whole-program step reads who may call what.
    let mut public = program.clone();
    let module = &mut public.modules[0];
    let library = module.functions.iter().any(|one| one.linkage == model::FunctionLinkage::External);
    for function in &mut module.functions {
        function.symbol = Some(object_name(function));
    }
    match entry.map(|entry| (entry, module.functions.iter_mut().find(|one| one.name == entry))) {
        Some((_, Some(function))) => function.linkage = model::FunctionLinkage::External,
        Some((_, None)) if library => {}
        Some((entry, None)) => return Err(format!("entry function {} does not exist", pyrepr::string(entry))),
        None => {}
    }
    let mut compiled = llrm_core::driver::compiled(&public, options)?.swap_remove(0);
    compiled.stack = llrm_core::backend::stackusage::stack_to_add(
        &compiled,
        os.stack_base,
        os.stack_reserve,
        llrm_core::backend::stackusage::stack_limit(options.arch.layout().segment_bytes()),
        &*options.arch,
    )?;
    Ok(compiled)
}

/// Makes each export no symbol in `used` names internal, so that it and
/// what only it calls are dropped: a linker's own elimination keeps
/// whatever any segment references, even one it drops.
pub fn keep_exports(
    program: &mut model::Program,
    used: &BTreeSet<String>,
) {
    for function in program.modules.iter_mut().flat_map(|module| module.functions.iter_mut()) {
        if function.linkage == model::FunctionLinkage::External && !used.contains(&function.name) {
            function.linkage = model::FunctionLinkage::Internal;
        }
    }
}

/// `target`'s machine, priced for its default CPU, with the far uninitialised data zeroed where the
/// target's OS layer under the runtime says its start-up does.
pub fn machine(
    target: &dyn llrm_target::Target,
    os: &crate::Os,
) -> llrm_core::abi::machine::Machine {
    llrm_core::abi::machine::Machine { far_bss: os.far_bss, ..target.machine() }
}

/// `module` as an object file of `format`, its code laid out as `layout` says.
pub fn object(
    module: &masm::Module,
    source: &Path,
    layout: objbuild::CodeLayout,
    format: llrm_target::object::Format,
) -> Result<Vec<u8>, String> {
    let name = source.file_name().map(|one| one.to_string_lossy().into_owned()).unwrap_or_default();
    objbuild::written_in(module, &name, layout, format).map_err(|error| error.to_string())
}
