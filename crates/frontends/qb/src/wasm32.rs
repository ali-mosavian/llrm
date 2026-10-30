//! `--wasm32`: the QB module through the shared mid-end as for a 32-bit flat target, then, as LLVM IR
//! text, through the wasm backend (`llrm_wasm::driver`) to a relocatable object.
//!
//! The text step is the host hand-over of `--arm64`: the runtime entries are the C names (`B_X`) of
//! runtime/qb, the module's exports are `QB_MAIN`, `QB_DATA` and `QB_STATEMENTS`. Wasm's callee
//! asserts a narrow argument already extended, as Darwin's does, so every `i16` argument of an entry
//! is `signext`.

use std::path::Path;

use llrm_codegen::driver::Options;
use llrm_codegen::hir::model;
use llrm_wasm::dag::Options as WasmOptions;
use llrm_wasm::driver;

use super::arm64::{
    EXPORTS, block_entry, framed, host_module, renamed, signext_i16_arguments, without_results_on_void,
};
use super::compile::CompileError;

/// `line` without the `cc 10NN` of the x86 convention it was lowered for.
fn without_convention(line: &str) -> String {
    let mut out = line.to_owned();
    while let Some(at) = out.find(" cc 10") {
        let digits = out[at + 6..].bytes().take_while(u8::is_ascii_digit).count();
        out.replace_range(at..at + 6 + digits, "");
    }
    out
}

/// A constant address `getelementptr (i8, ptr @X, i16 N)` indexes by the 16-bit word the mid-end has
/// for an offset; the pointer is 32 bits.
fn offsets_at_pointer_width(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("getelementptr (i8, ptr @") {
        let end = rest[at..].find(')').map_or(rest.len(), |close| at + close);
        let call = &rest[at..end];
        out.push_str(&rest[..at]);
        match call.rfind(", i16 ") {
            Some(index) => out.push_str(&format!("{}, i32 {}", &call[..index], &call[index + ", i16 ".len()..])),
            None => out.push_str(call),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The module as the wasm backend reads it: LLVM IR text.
fn wasm32_text(
    mir: &str,
    read_data: &[u8],
) -> String {
    let mut out = String::with_capacity(mir.len());
    for line in mir.lines() {
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target datalayout = \"{}\"\n", driver::datalayout(llrm_wasm::Width::W32)));
            continue;
        }
        let mut line = without_convention(line).replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = line.replace("@llrm.qb.B$", "@B_").replace("@B$", "@B_");
        line = signext_i16_arguments(&block_entry(&line));
        for (from, to) in EXPORTS {
            line = renamed(&line, from, to);
        }
        line = offsets_at_pointer_width(&line.replace("@$", "@Q_"));
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
    let mut out = framed(&without_results_on_void(&out));
    let bytes: String = read_data.iter().map(|byte| format!("\\{byte:02X}")).collect();
    out.push_str(&format!("@QB_DATA = constant [{} x i8] c\"{bytes}\"\n", read_data.len()));
    if !out.contains("@QB_STATEMENTS = ") {
        out.push_str("@QB_STATEMENTS = constant [0 x i8] zeroinitializer\n");
    }
    out
}

/// The wasm32 object of `program`.
pub fn object_bytes(
    program: &model::Program,
    _source: &Path,
    codegen: &Options,
) -> Result<Vec<u8>, CompileError> {
    let fail = |why: String| CompileError::Value(why);
    let (mir, read_data) = host_module(program, codegen)?;
    let text = wasm32_text(&mir, &read_data);
    if let Some(path) = std::env::var_os("LLRM_WASM32_TEXT") {
        std::fs::write(path, &text).map_err(|error| fail(error.to_string()))?;
    }
    driver::object(&text, llrm_wasm::Width::W32, WasmOptions::default()).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::wasm32_text;

    /// `PRINT "hello"` through the 32-bit mid-end: the x86 `cc 1002` and the 16-bit constant offset
    /// stayed in the text, which wasm's reader took for a convention and a sign-extended index.
    #[test]
    fn the_text_has_no_x86_convention_and_a_pointer_width_offset() {
        let mir = "target datalayout = \"e-m:e-p:32:32\"\n\
            @$string3 = internal constant <{ [4 x i8], ptr }> <{ [4 x i8] c\"\\05\\00\\00\\00\", ptr getelementptr (i8, ptr @$string3, i16 8) }>\n\
            define internal void @__main() {\nb1:\n  tail call cc 1002 void @llrm.qb.B$PESD(ptr @$string3)\n  ret void\n}\n\
            declare cc 1002 void @llrm.qb.B$PESD(ptr) nocallback\n";
        let text = wasm32_text(mir, &[0xff, 0xff, 0x01]);
        assert!(!text.contains(" cc 10"), "{text}");
        assert!(text.contains("getelementptr (i8, ptr @Q_string3, i32 8)"), "{text}");
        assert!(text.contains("define void @QB_MAIN()") && text.contains("call void @B_PESD(ptr @Q_string3)"), "{text}");
        assert!(text.contains("@QB_DATA = constant [3 x i8]") && text.contains("@QB_STATEMENTS"), "{text}");
    }
}
