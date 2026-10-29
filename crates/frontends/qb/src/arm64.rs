//! `--arm64=linux|darwin`: the QB module through the shared mid-end as for any
//! 64-bit target, then, as MIR text, through the arm64 backend
//! (`llrm_arm64::driver`) to an ELF or Mach-O object.
//!
//! The text step is the hand-over: the module leaves the x86 runtime lowering
//! (`cc 1000` calls, `llrm.qb.B$X` names, the x87 evaluation attribute) as C
//! calls and C names (`B_X`) of the host runtime in runtime/qb. The module's
//! exports are `QB_MAIN`, `QB_DATA` (the first DATA row) and `QB_STATEMENTS`
//! (the statement table), which runtime/qb/posix reads.

use std::path::Path;

use llrm_arm64::machine::Platform;
use llrm_codegen::driver::Options;
use llrm_codegen::hir::model;

use super::compile::{_inline_frame, _near_procedures, _positional_data, CompileError};

/// The names the host runtime links the module by.
const EXPORTS: [(&str, &str); 2] = [("@__main", "@QB_MAIN"), ("@$qb$statementTable", "@QB_STATEMENTS")];

/// `line` with the symbol `from` (a whole name, not the start of a longer one)
/// written `to`.
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

/// `line`, a call or declaration of a runtime entry (`@B_X`), with its `i16`
/// arguments marked `signext`. Darwin's AAPCS64 has the caller extend an
/// argument narrower than 32 bits, and the clang-built runtime relies on it:
/// `-1` passed as 0xFFFF reached `VIEW PRINT` as 65535, "Illegal function
/// call". Linux's callee extends for itself.
fn signext_i16_arguments(line: &str) -> String {
    let Some(name) = line.find("@B_") else { return line.to_owned() };
    let Some(open) = line[name..].find('(').map(|at| name + at) else { return line.to_owned() };
    let Some(close) = line[open..].find(')').map(|at| open + at) else { return line.to_owned() };
    let arguments: Vec<String> = line[open + 1..close]
        .split(", ")
        .map(|argument| match argument.strip_prefix("i16") {
            // already extended: the HIR marks the narrow argument of a call at
            // a 32-bit word
            Some(rest) if rest.trim_start().starts_with("signext") => argument.to_owned(),
            Some(rest) if rest.is_empty() || rest.starts_with(' ') => format!("i16 signext{rest}"),
            _ => argument.to_owned(),
        })
        .collect();
    format!("{}{}{}", &line[..=open], arguments.join(", "), &line[close..])
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
        // a variadic call has no prototype to extend its arguments: on every
        // platform the callee reads an int
        if platform == Platform::Darwin || line.contains(", ...) @B_") {
            line = signext_i16_arguments(&line);
        }
        line = aligned(&line);
        for (from, to) in EXPORTS {
            line = renamed(&line, from, to);
        }
        // the module's other symbols are private to it, but for what the
        // runtime names
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
    let mut out = framed(&without_results_on_void(&out));
    // an empty table may have been dropped as unused: the runtime still links
    // against it the READ/DATA rows: a position and the row's text for
    // each, then the end
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
    let (mut mir, _) = llrm_codegen::driver::emitted(&program, codegen).map_err(fail)?;
    llrm_codegen::driver::optimized(&mut mir, codegen).map_err(fail)?;
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

/// ON ERROR's landing on a host: a function with a landing pad registers a
/// `_setjmp` buffer (`qb_host_frame`, which returns the frame it replaces and
/// gets it back before each `ret`), and the runtime's `qb_land` jumps back into
/// the function, which takes the branch to its pad. The function's allocas are
/// handed to `qb_host_escape` so that no pass keeps them in registers, which
/// the jump would restore stale.
fn framed(text: &str) -> String {
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
                out.push_str(&framed_function(&function));
                function.clear();
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !text.contains("declare i16 @B_FERR()") {
        out.push_str("declare i16 @B_FERR()\n");
    }
    out.push_str(
        "declare ptr @qb_host_frame(ptr)\ndeclare void @qb_host_escape(ptr)\ndeclare i32 @_setjmp(ptr) returns_twice\n",
    );
    out
}

fn framed_function(lines: &[&str]) -> String {
    let mut out = String::new();
    let landing = lines.iter().any(|line| line.trim_start().starts_with("%landed = landingpad"));
    let entry = lines.iter().position(|line| line.ends_with(':')).filter(|_| landing);
    let Some(entry) = entry else {
        return lines.iter().map(|line| format!("{line}\n")).collect();
    };
    let mut at = entry + 1;
    while lines[at].contains(" = alloca ") {
        at += 1;
    }
    let label = lines[entry].trim_end_matches(':');
    let from_entry = format!(", %{label} ]");
    let name = |line: &str| line.trim().split(' ').next().unwrap_or("").to_owned();
    for (number, line) in lines.iter().enumerate() {
        if number == at {
            for alloca in &lines[entry + 1..at] {
                out.push_str(&format!("  call void @qb_host_escape(ptr {})\n", name(alloca)));
            }
            out.push_str("  %qb.jb = alloca [64 x i64], align 16\n  call void @qb_host_escape(ptr %qb.jb)\n");
            out.push_str(
                "  %qb.prev = call ptr @qb_host_frame(ptr %qb.jb)\n  %qb.rc = call i32 @_setjmp(ptr %qb.jb)\n",
            );
            out.push_str(
                "  %qb.back = icmp ne i32 %qb.rc, 0\n  br i1 %qb.back, label %landing, label %qb.body\n\nqb.body:\n",
            );
        }
        if line.contains("= load i16, ptr @Q_QB$LANDED") {
            // DOS lands in `$QB$LANDING`, which stores ERR there; here the jump
            // comes straight to the pad
            out.push_str("  %qb.err = call i16 @B_FERR()\n  store i16 %qb.err, ptr @Q_QB$LANDED\n");
        }
        if line.trim_start().starts_with("ret ") {
            out.push_str("  %qb.popped = call ptr @qb_host_frame(ptr %qb.prev)\n");
        }
        // the rest of the entry block is the body now: a phi's edge from the
        // entry comes from there
        let line = if number > at { line.replace(&from_entry, ", %qb.body ]") } else { (*line).to_owned() };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// A mid-end pass leaves a result number on a terminator or a store (`%54 = br
/// label %b36`), which the x86 path never reads and MIR's reader refuses. Each
/// such number is dropped and the numbers after it in the function close up.
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

/// `line` with each numeric name (`%7`, a block label `7:`) lowered by the
/// dropped numbers below it.
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

/// The entries QB calls with a block of words on the stack (`B$DDIM`, `B$COLR`,
/// ...): a host's are variadic C functions (runtime/qb/posix/blockentries.c)
/// that read the block's words in memory order, which is the reverse of the
/// order they were pushed in.
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

/// QB packs its variables, and the x86 loads what it finds; arm64's `ldr x` of
/// a symbol needs it 8 aligned for the relocation
/// (`R_AARCH64_LDST64_ABS_LO12_NC`). Every module global starts on 8 (16 from
/// 16 bytes).
fn aligned(line: &str) -> String {
    if !line.starts_with('@') || !(line.contains(" global ") || line.contains(" constant ")) {
        return line.to_owned();
    }
    let body = match line.rfind(", align ") {
        Some(at) => &line[..at],
        None => line,
    };
    format!("{body}, align 16")
}

#[cfg(test)]
mod tests {
    use super::signext_i16_arguments;

    /// The call already carried `signext` (the HIR extends a 16-bit argument at
    /// a 32-bit word), and Darwin's pass added a second: `i16 signext signext
    /// -1` (qb_arm64 darwin_callers_extend_narrow_arguments_to_the_runtime).
    #[test]
    fn an_argument_already_extended_is_not_extended_twice() {
        let line = "  tail call void @B_PEI2(i16 signext -1)";
        assert_eq!(signext_i16_arguments(line), line);
        assert_eq!(
            signext_i16_arguments("  tail call void @B_PEI2(i16 -1, i16 signext 2)"),
            "  tail call void @B_PEI2(i16 signext -1, i16 signext 2)"
        );
    }
}
