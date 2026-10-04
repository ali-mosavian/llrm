//! Port of `qbopt/frontend/modern/compile.py`: common HIR from the modern
//! frontend to a fresh 16-bit OMF object.

use std::collections::BTreeSet;
use std::path::Path;

use llrm_core::backend::{masm, omfwrite};
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
pub fn assembled(program: &model::Program, entry: &str, options: &llrm_core::driver::Options) -> Result<masm::Module, String> {
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
    match module.functions.iter_mut().find(|one| one.name == entry) {
        Some(function) => function.linkage = model::FunctionLinkage::External,
        None if library => {}
        None => return Err(format!("entry function {} does not exist", pyrepr::string(entry))),
    }
    let mut compiled = llrm_core::driver::compiled(&public, options)?.swap_remove(0);
    compiled.stack = llrm_core::backend::stackusage::stack_to_add(&compiled, STACK_BASE)?;
    Ok(compiled)
}

/// The stack `runtime/start.asm` reserves; the object's own adds to it.
pub const STACK_BASE: i64 = 4096;

/// Makes each export no symbol in `used` names internal, so that it and
/// what only it calls are dropped: a linker's own elimination keeps
/// whatever any segment references, even one it drops.
pub fn keep_exports(program: &mut model::Program, used: &BTreeSet<String>) {
    for function in program.modules.iter_mut().flat_map(|module| module.functions.iter_mut()) {
        if function.linkage == model::FunctionLinkage::External && !used.contains(&function.name) {
            function.linkage = model::FunctionLinkage::Internal;
        }
    }
}

/// The processor objects are compiled for.
pub const CPU: &str = "486";

/// The built-in machine, priced for `CPU`.
pub fn machine() -> llrm_core::abi::machine::Machine {
    // start.asm zeroes the far uninitialised data, as it does the near.
    llrm_core::abi::machine::Machine { cpu: CPU.to_owned(), far_bss: true, ..llrm_core::abi::machine::BUILT_IN.clone() }
}

/// `module` as an OMF object, its code laid out as `layout` says.
pub fn object(module: &masm::Module, source: &Path, layout: omfwrite::CodeLayout) -> Result<Vec<u8>, String> {
    let name = source.file_name().map(|one| one.to_string_lossy().into_owned()).unwrap_or_default();
    omfwrite::written_as(module, &name, layout).map_err(|error| error.to_string())
}

