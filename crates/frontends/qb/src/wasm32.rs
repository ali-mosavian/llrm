//! `--wasm32`: the QB module through the shared mid-end as for a 32-bit flat
//! target, then, as LLVM IR text, through the wasm backend
//! (`llrm_wasm::driver`) to a relocatable object.
//!
//! The text step is the host hand-over of `--arm64`: the runtime entries are
//! the C names (`B_X`) of runtime/qb, the module's exports are `QB_MAIN`,
//! `QB_DATA` and `QB_STATEMENTS`. Wasm's callee asserts a narrow argument
//! already extended, as Darwin's does, so every `i16` argument of an entry
//! is `signext`.
//!
//! DEVIATE W16: the module still goes through LLVM IR text with `--arm64`'s
//! name mapping (the runtime's `B_X` names, the exports, the `Q_` prefix).
//! Reason: the symbol spelling of an object format is not in the
//! target description. Exit: that spelling in the object format's description,
//! then the module in memory. The conventions (`cc 10NN`) and 16-bit offsets of
//! the 32-bit mid-end are no longer rewritten: the offset is as wide as the
//! near pointer and the wasm backend has one convention.
//!
//! LIMIT W17: wasm32 has no extended float, so `x ^ y` is the runtime's `pow`
//! in doubles (`B$LPOW`, keyed on a datalayout with no `f80`), not the x87
//! sequence in 80 bits. Its digits differ from BC's in the last place, three
//! ulp at most on the programs of tests/run/qb that print them; no exit.

use std::path::Path;

use llrm_codegen::driver::Options;
use llrm_codegen::hir::model;
use llrm_wasm::dag::{ExceptionModel, Options as WasmOptions};
use llrm_wasm::driver;

use super::arm64::{
    EXPORTS, block_entry, host_module, ports_as_calls, renamed, signext_i16_arguments, without_llrm_attributes,
    without_results_on_void,
};
use super::compile::CompileError;

/// The module as the wasm backend reads it: LLVM IR text.
fn wasm32_text(
    mir: &str,
    read_data: &[u8],
) -> Result<String, String> {
    let mut out = String::with_capacity(mir.len());
    for line in mir.lines() {
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target datalayout = \"{}\"\n", driver::datalayout(llrm_wasm::Width::W32)));
            continue;
        }
        let mut line = line.replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = ports_as_calls(&without_llrm_attributes(&line));
        line = line.replace("@llrm.qb.B$", "@B_").replace("@B$", "@B_");
        line = signext_i16_arguments(&block_entry(&line));
        for (from, to) in EXPORTS {
            line = renamed(&line, from, to);
        }
        line = line.replace("@$", "@Q_");
        if let Some(rest) = line.strip_prefix("define internal ").filter(|_| line.contains("@QB_MAIN")) {
            line = format!("define {rest}");
        }
        let statements = "@QB_STATEMENTS = internal ";
        if line.starts_with(statements) {
            line = line.replacen(" internal ", " ", 1);
        }
        out.push_str(&line);
        out.push('\n');
    }
    // The module keeps its `invoke`s and `landingpad`s for the backend.
    let mut out = without_results_on_void(&out);
    let bytes: String = read_data.iter().map(|byte| format!("\\{byte:02X}")).collect();
    out.push_str(&format!("@QB_DATA = constant [{} x i8] c\"{bytes}\"\n", read_data.len()));
    if !out.contains("@QB_STATEMENTS = ") {
        out.push_str("@QB_STATEMENTS = constant [0 x i8] zeroinitializer\n");
    }
    Ok(out)
}

/// The wasm32 object of `program`.
pub fn object_bytes(
    program: &model::Program,
    _source: &Path,
    codegen: &Options,
) -> Result<Vec<u8>, CompileError> {
    let fail = |why: String| CompileError::Value(why);
    let (mir, read_data) = host_module(program, codegen)?;
    let text = wasm32_text(&mir, &read_data).map_err(fail)?;
    if let Some(path) = std::env::var_os("LLRM_WASM32_TEXT") {
        std::fs::write(path, &text).map_err(|error| fail(error.to_string()))?;
    }
    driver::object(
        &text,
        llrm_wasm::Width::W32,
        WasmOptions { exception_model: ExceptionModel::Wasm, ..WasmOptions::default() },
    )
    .map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::wasm32_text;

    /// `-fsanitize=stack` marks a routine `stackcheck`, twice in a row; the
    /// reader refused the text ("expected `{`, found `stackcheck`") and every
    /// stack-checked program failed to build for wasm32. Facts only llrm reads
    /// are not handed over, as for arm64.
    #[test]
    fn attributes_only_llrm_reads_are_not_handed_over() {
        let mir = "define cc 1000 i16 @\"DEEP%\"(ptr %0) stackcheck stackcheck nounwind {\nb1:\n  ret i16 0\n}\n";
        let text = wasm32_text(mir, &[]).unwrap();
        assert!(!text.contains("stackcheck") && text.contains("nounwind {"), "{text}");
    }

    /// `OUT` and `INP` stayed `llrm.ia16.out.i8` and `llrm.ia16.in.i8`: wasm
    /// selects no port node, and arm64 linked an undefined symbol. The text
    /// calls the runtime's `qb_port_out` and `qb_port_in`.
    #[test]
    fn port_intrinsics_are_calls_of_the_runtimes_port_routines() {
        let mir = "declare void @llrm.ia16.out.i8(i16, i8)\ndeclare i8 @llrm.ia16.in.i8(i16)\n\
            define internal void @__main() {\nb1:\n  call void @llrm.ia16.out.i8(i16 968, i8 7)\n  %v = call i8 @llrm.ia16.in.i8(i16 969)\n  ret void\n}\n";
        let text = wasm32_text(mir, &[], false).unwrap();
        assert!(!text.contains("llrm.ia16"), "{text}");
        assert!(
            text.contains("declare void @qb_port_out(i16, i8)") && text.contains("call i8 @qb_port_in(i16 969)"),
            "{text}"
        );
    }

    /// `PRINT "hello"` through the 32-bit mid-end: the module's names are the
    /// runtime's and its exports the host's; the text keeps the mid-end's
    /// conventions and pointer-wide offsets as they are.
    #[test]
    fn the_text_has_the_hosts_names_and_the_mid_ends_conventions_and_offsets() {
        let mir = "target datalayout = \"e-m:e-p:32:32\"\n\
            @$string3 = internal constant <{ [4 x i8], ptr }> <{ [4 x i8] c\"\\05\\00\\00\\00\", ptr getelementptr (i8, ptr @$string3, i32 8) }>\n\
            define internal void @__main() {\nb1:\n  tail call cc 1002 void @llrm.qb.B$PESD(ptr @$string3)\n  ret void\n}\n\
            declare cc 1002 void @llrm.qb.B$PESD(ptr) nocallback\n";
        let text = wasm32_text(mir, &[0xff, 0xff, 0x01]).unwrap();
        assert!(text.contains("getelementptr (i8, ptr @Q_string3, i32 8)"), "{text}");
        assert!(
            text.contains("define void @QB_MAIN()") && text.contains("call cc 1002 void @B_PESD(ptr @Q_string3)"),
            "{text}"
        );
        assert!(text.contains("@QB_DATA = constant [3 x i8]") && text.contains("@QB_STATEMENTS"), "{text}");
    }
}
