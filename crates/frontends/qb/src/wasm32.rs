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
//! name mapping (the runtime's `B_X` names, the exports, the `Q_` prefix,
//! `framed`). Reason: the symbol spelling of an object format is not in the
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
    EXPORTS, block_entry, framed, host_module, renamed, signext_i16_arguments, without_llrm_attributes,
    without_results_on_void,
};
use super::compile::CompileError;

/// The module as the wasm backend reads it: LLVM IR text.
fn wasm32_text(
    mir: &str,
    read_data: &[u8],
    exceptions: bool,
) -> Result<String, String> {
    let mut out = String::with_capacity(mir.len());
    for line in mir.lines() {
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target datalayout = \"{}\"\n", driver::datalayout(llrm_wasm::Width::W32)));
            continue;
        }
        let mut line = line.replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = without_llrm_attributes(&line);
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
    // With exception handling the module keeps its `invoke`s and `landingpad`s
    // for the backend; otherwise a function with a handler registers a
    // `setjmp` frame.
    let mut out =
        if exceptions { unwinding(&without_results_on_void(&out))? } else { framed(&without_results_on_void(&out)) };
    let bytes: String = read_data.iter().map(|byte| format!("\\{byte:02X}")).collect();
    out.push_str(&format!("@QB_DATA = constant [{} x i8] c\"{bytes}\"\n", read_data.len()));
    if !out.contains("@QB_STATEMENTS = ") {
        out.push_str("@QB_STATEMENTS = constant [0 x i8] zeroinitializer\n");
    }
    Ok(out)
}

/// ON ERROR's landing with wasm exception handling: the runtime throws, and a
/// function with a landing pad catches it with `catch_all`. What the pad does
/// next, a branch back into the function, cannot leave a `catch`, so the
/// function becomes a loop around its body:
///
/// ```text
/// entry:  allocas; resume = -1; br loop          entry:  allocas ...
/// start:  the rest of the entry block            loop:   switch resume [k: re-entry k, else start]
/// pad:    ...; br/switch to blocks               pad:    ...; the same, each target a block
/// body                                           re.k:   resume = k; br loop
/// ```
///
/// so the `try` begins at the loop's head, which dominates every call that
/// unwinds to the pad, and the pad ends in a `br` to the loop. The pad reads
/// ERR from the runtime as `framed` does.
fn unwinding(text: &str) -> Result<String, String> {
    let mut out = String::with_capacity(text.len());
    let mut function: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("define ") {
            inside = true;
        }
        if inside {
            function.push(line);
            if line == "}" {
                inside = false;
                out.push_str(&unwinding_function(&function)?);
                function.clear();
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !text.lines().any(|line| line.starts_with("declare ") && line.contains("@B_FERR(")) {
        out.push_str("declare i16 @B_FERR()\n");
    }
    Ok(out)
}

/// The labels a terminator names, once each, in order.
fn labels_of(text: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for part in text.split("label %").skip(1) {
        let name: String = part.chars().take_while(|c| !matches!(c, ',' | ' ' | ']' | '\n' | ')')).collect();
        if !found.contains(&name) {
            found.push(name);
        }
    }
    found
}

/// `line` with each `label %T` of `targets` (the `k`-th) naming `qb.re<k>`.
fn trampolined(
    line: &str,
    targets: &[String],
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find("label %") {
        let name_at = at + "label %".len();
        out.push_str(&rest[..name_at]);
        rest = &rest[name_at..];
        let end = rest.find(|c: char| matches!(c, ',' | ' ' | ']' | ')')).unwrap_or(rest.len());
        match targets.iter().position(|target| target == &rest[..end]) {
            Some(k) => out.push_str(&format!("qb.re{k}")),
            None => out.push_str(&rest[..end]),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// `lines` with the pad's block cut before its `invoke` at `call`: the pad ends
/// in a branch to `qb.cont`, which starts with the call.
fn split_pad(
    lines: &[&str],
    pad: &str,
    call: usize,
    pad_end: usize,
) -> Result<String, String> {
    let pad_at = lines[..call].iter().rposition(|line| !line.starts_with(' ') && line.ends_with(':')).expect("the pad");
    let defined: Vec<&str> = lines[pad_at..call]
        .iter()
        .filter_map(|line| {
            line.trim_start().split(" = ").next().filter(|name| name.starts_with('%')).filter(|_| line.contains(" = "))
        })
        .collect();
    let tail = lines[call..pad_end].join("\n");
    let uses = |name: &str| {
        let mut rest = tail.as_str();
        while let Some(at) = rest.find(name) {
            let after = rest[at + name.len()..].chars().next();
            if !after.is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '$')) {
                return true;
            }
            rest = &rest[at + name.len()..];
        }
        false
    };
    if let Some(name) = defined.iter().find(|name| uses(name)) {
        return Err(format!("a landing pad value ({name}) is used after the call the pad re-arms with"));
    }
    let successors = labels_of(&tail);
    let mut split: Vec<String> = Vec::with_capacity(lines.len() + 4);
    for (index, line) in lines.iter().enumerate() {
        if index == call {
            split.push("  br label %qb.cont".to_owned());
            split.push(String::new());
            split.push("qb.cont:".to_owned());
        }
        split.push((*line).to_owned());
    }
    // The phis of the call's normal destination take the edge from `qb.cont`.
    let mut current = String::new();
    for line in &mut split {
        if !line.starts_with(' ') && line.ends_with(':') {
            current = line.trim_end_matches(':').to_owned();
        }
        if line.contains(" = phi ") && successors.iter().any(|one| *one == current && one != pad) {
            *line = line.replace(&format!(", %{pad} ]"), ", %qb.cont ]");
        }
    }
    let joined: Vec<&str> = split.iter().map(String::as_str).collect();
    unwinding_function(&joined)
}

fn unwinding_function(lines: &[&str]) -> Result<String, String> {
    let plain = || Ok(lines.iter().map(|line| format!("{line}\n")).collect::<String>());
    let is_label = |line: &str| !line.starts_with(' ') && line.ends_with(':');
    let Some(landing) = lines.iter().position(|line| line.trim_start().starts_with("%landed = landingpad")) else {
        return plain();
    };
    let Some(entry) = lines.iter().position(|line| is_label(line)) else { return plain() };
    let pad_at = lines[..landing].iter().rposition(|line| is_label(line)).expect("the pad's label");
    let pad = lines[pad_at].trim_end_matches(':').to_owned();
    let entry_name = lines[entry].trim_end_matches(':').to_owned();
    // The pad block ends at the next label or the closing brace.
    let pad_end =
        lines[landing..].iter().position(|line| is_label(line) || *line == "}").map_or(lines.len(), |at| landing + at);
    // A pad that calls with the pad as its own unwind destination (ON LOCAL
    // ERROR re-arms in the handler) continues in a block of the body: the part
    // from that call on is entered through the loop like any re-entry.
    if let Some(call) = lines[landing..pad_end].iter().position(|line| {
        let line = line.trim_start();
        line.starts_with("invoke ") || line.contains(" = invoke ")
    }) {
        return split_pad(lines, &pad, landing + call, pad_end);
    }
    let terminator_at = lines[pad_at..pad_end]
        .iter()
        .rposition(
            |line| {
                let line = line.trim_start();
                line.starts_with("br ")
                    || line.starts_with("switch ")
                    || line.starts_with("ret ")
                    || line.starts_with("unreachable")
            },
        );
    let terminator_at = pad_at
        + terminator_at
            .unwrap_or_else(|| panic!("the pad ends in a terminator: {}", lines[pad_at..pad_end].join("\n")));
    // A `switch` runs over lines until its `]`.
    let mut block_end = terminator_at + 1;
    if lines[terminator_at].trim_end().ends_with('[') {
        while !lines[block_end - 1].trim().starts_with(']') {
            block_end += 1;
        }
    }
    let targets = labels_of(&lines[terminator_at..block_end].join("\n"));
    let mut at = entry + 1;
    while lines[at].contains(" = alloca ") {
        at += 1;
    }
    // The entry's successors, whose phis name the entry.
    let entry_end = lines[entry..].iter().position(|line| line.is_empty()).map_or(lines.len(), |end| entry + end);
    let entry_targets = labels_of(&lines[at..entry_end].join("\n"));
    let mut out = String::new();
    let mut current = String::new();
    let from = |label: &str| format!(", %{label} ]");
    let rename = |line: &str, current: &str| -> String {
        let mut line = line.to_owned();
        if line.contains(" = phi ") {
            if targets.iter().any(|one| one == current) {
                line = line.replace(&from(&pad), ", %qb.loop ]");
            }
            if entry_targets.iter().any(|one| one == current) {
                line = line.replace(&from(&entry_name), ", %qb.start ]");
            }
        }
        line
    };
    for (index, line) in lines.iter().enumerate() {
        if is_label(line) {
            current = line.trim_end_matches(':').to_owned();
        }
        if index == at {
            out.push_str(
                "  %qb.resume = alloca i32\n  store i32 -1, ptr %qb.resume\n  br label %qb.loop\n\nqb.loop:\n",
            );
            out.push_str("  %qb.next = load i32, ptr %qb.resume\n  switch i32 %qb.next, label %qb.start [\n");
            for (k, target) in targets.iter().enumerate() {
                out.push_str(&format!("    i32 {k}, label %{target}\n"));
            }
            out.push_str("  ]\n\nqb.start:\n");
        }
        if line.contains("= load i16, ptr @Q_QB$LANDED") {
            out.push_str("  %qb.err = call i16 @B_FERR()\n  store i16 %qb.err, ptr @Q_QB$LANDED\n");
        }
        if index >= terminator_at && index < block_end {
            out.push_str(&trampolined(line, &targets));
            out.push('\n');
            if index + 1 == block_end {
                for k in 0..targets.len() {
                    out.push_str(&format!("\nqb.re{k}:\n  store i32 {k}, ptr %qb.resume\n  br label %qb.loop\n"));
                }
            }
            continue;
        }
        out.push_str(&rename(line, &current));
        out.push('\n');
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
    let exceptions = codegen.wasm_exceptions;
    let text = wasm32_text(&mir, &read_data, exceptions).map_err(fail)?;
    if let Some(path) = std::env::var_os("LLRM_WASM32_TEXT") {
        std::fs::write(path, &text).map_err(|error| fail(error.to_string()))?;
    }
    let model = if exceptions { ExceptionModel::Wasm } else { ExceptionModel::None };
    driver::object(
        &text,
        llrm_wasm::Width::W32,
        WasmOptions { exception_model: model, calls_unwind: !exceptions, ..WasmOptions::default() },
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
        let text = wasm32_text(mir, &[], false).unwrap();
        assert!(!text.contains("stackcheck") && text.contains("nounwind {"), "{text}");
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
        let text = wasm32_text(mir, &[0xff, 0xff, 0x01], false).unwrap();
        assert!(text.contains("getelementptr (i8, ptr @Q_string3, i32 8)"), "{text}");
        assert!(
            text.contains("define void @QB_MAIN()") && text.contains("call cc 1002 void @B_PESD(ptr @Q_string3)"),
            "{text}"
        );
        assert!(text.contains("@QB_DATA = constant [3 x i8]") && text.contains("@QB_STATEMENTS"), "{text}");
    }
}
