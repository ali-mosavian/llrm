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

/// `line` with the symbol `from` (a whole name, not the start of a longer one) written `to`.
fn renamed(
    line: &str,
    from: &str,
    to: &str,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(from) {
        let after = &rest[at + from.len()..];
        let whole = !after.starts_with(|c: char| c.is_alphanumeric() || matches!(c, '$' | '_' | '.'));
        out.push_str(&rest[..at]);
        out.push_str(if whole { to } else { from });
        rest = after;
    }
    out.push_str(rest);
    out
}

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
        let mut line = line.replace(" cc 1000", "").replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = line.replace("@llrm.qb.B$", "@B_");
        for (from, to) in EXPORTS {
            line = renamed(&line, from, to);
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
    let mut out = without_results_on_void(&out);
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
    if let Some(path) = std::env::var_os("LLRM_ARM64_TEXT") {
        std::fs::write(path, &text).map_err(|error| fail(error.to_string()))?;
    }
    let name = source.file_stem().map_or_else(String::new, |one| one.to_string_lossy().into_owned());
    llrm_arm64::driver::object_file(&text, platform, &name, false).map_err(fail)
}

/// A mid-end pass leaves a result number on a terminator or a store (`%54 = br label %b36`), which the x86 path never
/// reads and MIR's reader refuses. Each such number is dropped and the numbers after it in the function close up.
fn without_results_on_void(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut body: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with("define ") {
            body.clear();
        }
        if body.is_empty() && !line.starts_with("define ") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        body.push(line);
        if line == "}" {
            out.push_str(&closed_up(&body));
            body.clear();
        }
    }
    out
}

fn is_void(rest: &str) -> bool {
    ["br ", "ret ", "switch ", "unreachable", "store "].iter().any(|void| rest.starts_with(void))
}

fn closed_up(lines: &[&str]) -> String {
    let mut dropped: Vec<u64> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        if let Some((name, rest)) = trimmed.split_once(" = ")
            && let Some(number) = name.strip_prefix('%').and_then(|digits| digits.parse::<u64>().ok())
            && is_void(rest)
        {
            dropped.push(number);
        }
    }
    let mut out = String::new();
    for line in lines {
        let trimmed = line.trim_start();
        let line = match trimmed.split_once(" = ") {
            Some((name, rest))
                if name.strip_prefix('%').is_some_and(|digits| digits.parse::<u64>().is_ok()) && is_void(rest) =>
            {
                format!("  {rest}")
            }
            _ => (*line).to_owned(),
        };
        out.push_str(&if dropped.is_empty() { line } else { shifted(&line, &dropped) });
        out.push('\n');
    }
    out
}

/// `line` with each numeric name (`%7`, a block label `7:`) lowered by the dropped numbers below it.
fn shifted(
    line: &str,
    dropped: &[u64],
) -> String {
    let lower = |number: u64| number - dropped.iter().filter(|one| **one < number).count() as u64;
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut at = 0;
    while at < bytes.len() {
        let numbered = |from: usize| {
            let digits = line[from..].bytes().take_while(u8::is_ascii_digit).count();
            (digits > 0).then(|| (line[from..from + digits].parse::<u64>().unwrap_or(u64::MAX), digits))
        };
        if bytes[at] == b'%' && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric()) {
            if let Some((number, digits)) = numbered(at + 1)
                && !line[at + 1 + digits..].starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '.')
            {
                out.push_str(&format!("%{}", lower(number)));
                at += 1 + digits;
                continue;
            }
        }
        if at == 0 {
            if let Some((number, digits)) = numbered(0).filter(|(_, digits)| line[*digits..].starts_with(':')) {
                out.push_str(&format!("{}", lower(number)));
                at += digits;
                continue;
            }
        }
        let c = line[at..].chars().next().unwrap_or(' ');
        out.push(c);
        at += c.len_utf8();
    }
    out
}
