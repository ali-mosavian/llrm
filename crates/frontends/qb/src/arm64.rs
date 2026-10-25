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
const EXPORTS: [(&str, &str); 2] = [("@__main", "@QB_MAIN"), ("@$qb$statementTable", "@QB_STATEMENTS")];

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
    read_data: &[u8],
) -> String {
    let mut out = String::with_capacity(mir.len());
    for line in mir.lines() {
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target datalayout = \"{}\"\n", platform.datalayout()));
            continue;
        }
        let mut line = line.replace(" cc 1000", "").replace(" \"llrm-fp-eval\"=\"x87-extended\"", "");
        line = line.replace("@llrm.qb.B$", "@B_").replace("@B$", "@B_");
        line = block_entry(&line);
        for (from, to) in EXPORTS {
            line = renamed(&line, from, to);
        }
        // the module's other symbols are private to it, but for what the runtime names
        line = line.replace("@$", "@Q_").replace("i16 16)", "i64 16)");
        if let Some(rest) = line.strip_prefix("define internal ").filter(|_| line.contains("@QB_MAIN")) {
            line = format!("define {rest}");
        }
        for name in ["QB_STATEMENTS"] {
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
    // the READ/DATA rows: a position and the row's text for each, then the end
    let bytes: String = read_data.iter().map(|byte| format!("\\{byte:02X}")).collect();
    out.push_str(&format!("@QB_DATA = constant [{} x i8] c\"{bytes}\"\n", read_data.len()));
    for name in ["QB_STATEMENTS"] {
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
    let mut read_data = Vec::new();
    for (position, line) in super::compile::_read_data_lines(&program.modules[0])?.into_iter().enumerate() {
        read_data.extend((position as u16).to_le_bytes());
        read_data.extend(line);
        read_data.push(0);
    }
    read_data.extend([0xff, 0xff, 0x01]);
    let text = arm64_text(&llrm_mir::print::module(&mir.modules[0]), platform, &read_data);
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

/// The entries QB calls with a block of words on the stack (`B$DDIM`, `B$COLR`, ...): a host's are variadic C functions
/// (runtime/qb/posix/blockentries.c) that read the block's words in memory order, which is the reverse of the order they
/// were pushed in.
const BLOCK_ENTRIES: [(&str, &str); 7] = [
    ("B_DDIM", "ptr"),
    ("B_RDIM", "ptr"),
    ("B_RDMP", "ptr"),
    ("B_CLOS", "i16"),
    ("B_COLR", "i16"),
    ("B_LOCT", "i16"),
    ("B_CSCN", "i16"),
];

fn block_entry(line: &str) -> String {
    for (name, first) in BLOCK_ENTRIES {
        let symbol = format!("@{name}(");
        let Some(at) = line.find(&symbol) else { continue };
        let args_at = at + symbol.len();
        if line.trim_start().starts_with("declare ") {
            let close = line[args_at..].find(')').map_or(line.len(), |one| args_at + one);
            return format!("{}{first}, ...{}", &line[..args_at], &line[close..]);
        }
        let mut depth = 0usize;
        let mut close = line.len();
        let mut parts = Vec::new();
        let mut from = args_at;
        for (index, c) in line[args_at..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' if depth == 0 => {
                    parts.push(line[from..args_at + index].trim());
                    close = args_at + index;
                    break;
                }
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    parts.push(line[from..args_at + index].trim());
                    from = args_at + index + 1;
                }
                _ => {}
            }
        }
        parts.reverse();
        let head = line[..at].trim_end();
        let head = head.strip_suffix("void").map_or(head.to_owned(), |before| format!("{before}void ({first}, ...)"));
        return format!("{head} {symbol}{}{}", parts.join(", "), &line[close..]);
    }
    line.to_owned()
}
