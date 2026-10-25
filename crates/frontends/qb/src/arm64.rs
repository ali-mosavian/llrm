//! `--arm64=linux|darwin`: the QB module through the shared mid-end as for any 64-bit target, then, as MIR text, through
//! the arm64 backend (`llrm_arm64::driver`) to an ELF or Mach-O object.
//!
//! The text step is the hand-over: the module leaves the x86 runtime lowering (`cc 1000` calls, `llrm.qb.B$X` names,
//! the x87 evaluation attribute) as C calls and C names (`B_X`) of the host runtime in runtime/qb. The module's exports
//! are `QB_MAIN`, `QB_DATA` (the first DATA row) and `QB_STATEMENTS` (the statement table), which runtime/qb/posix reads.

use std::path::Path;

use llrm_arm64::machine::Platform;
use llrm_core::hir::model;

use super::compile::{CompileError, _inline_frame, _near_procedures, _positional_data};
use llrm_core::driver::Options;

/// The names the host runtime links the module by.
const EXPORTS: [(&str, &str); 3] =
    [("@__main", "@QB_MAIN"), ("@$data", "@QB_DATA"), ("@$qb$statementTable", "@QB_STATEMENTS")];

/// The module as the arm64 backend reads it: LLVM IR text for `platform`.
fn arm64_text(
    mir: &str,
    platform: Platform,
) -> String {
    let mut out = String::with_capacity(mir.len());
    for line in mir.lines() {
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target datalayout = \"{}\"\n", platform.datalayout()));
            continue;
        }
        if line.starts_with('!') {
            continue;
        }
        let mut line = line.replace(" cc 1000", "").replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = line.replace("@llrm.qb.B$", "@B_");
        for (from, to) in EXPORTS {
            line = line.replace(&format!("{from} "), &format!("{to} ")).replace(&format!("{from}("), &format!("{to}("));
        }
        // the module's other symbols are private to it, but for what the runtime names
        line = line.replace("@$", "@Q_").replace("i16 16)", "i64 16)");
        if let Some(rest) = line.strip_prefix("define internal ").filter(|_| line.contains("@QB_MAIN")) {
            line = format!("define {rest}");
        }
        for name in ["QB_DATA", "QB_STATEMENTS"] {
            let internal = format!("@{name} = internal ");
            if line.starts_with(&internal) {
                line = line.replacen(" internal ", " ", 1);
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    // an empty table may have been dropped as unused: the runtime still links against it
    for name in ["QB_DATA", "QB_STATEMENTS"] {
        if !out.contains(&format!("@{name} = ")) {
            out.push_str(&format!("@{name} = constant [0 x i8] zeroinitializer\n"));
        }
    }
    out
}

/// The object of `program` for `platform`.
pub fn object_bytes(
    program: &model::Program,
    source: &Path,
    codegen: &Options,
    platform: Platform,
) -> Result<Vec<u8>, CompileError> {
    let fail = |why: String| CompileError::Value(why);
    let laid_out = super::zero_fill::laid_out(program, |module, function| !_inline_frame(program, module, function));
    let program = _near_procedures(&laid_out, codegen.arch.layout().spaces.far_is_near());
    if program.modules.len() != 1 {
        return Err(fail("one object represents exactly one QB module".into()));
    }
    let program = _positional_data(&program)?;
    let (mut mir, _) = llrm_core::driver::emitted(&program, codegen).map_err(fail)?;
    llrm_core::driver::optimized(&mut mir, codegen).map_err(fail)?;
    let text = arm64_text(&llrm_mir::print::module(&mir.modules[0]), platform);
    let name = source.file_stem().map_or_else(String::new, |one| one.to_string_lossy().into_owned());
    llrm_arm64::driver::object_file(&text, platform, &name, false).map_err(fail)
}
