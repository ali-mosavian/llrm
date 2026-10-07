//! LLVM's `opt` as the mid end: each module is printed, optimized by the
//! stock binary and read back, in place of this pipeline's machine-independent
//! passes. A measurement spike (`LLRM_LLVM_MID=O2|Os`), not a product.
//!
//! Conventions llvm has no name for go out as `cc N` and come back as N; the
//! parser already reads what `opt` writes, but for a few spellings it
//! refuses, which `imported` rewrites.

use std::path::Path;
use std::process::{Command, Stdio};

use llrm_mir::module::{GlobalValue, Linkage, Module};
use llrm_mir::opcode::CONVENTIONS;
use llrm_mir::program::Program;

/// What to run `opt` with.
pub struct Mid {
    pub level: String,
    pub opt: String,
    pub triple: String,
    pub cpu: String,
    /// `-inline-threshold`, and `noinline` on what the program exports when it is 0.
    pub inline_limit: i64,
    pub extra: Vec<String>,
    /// The `n` of the datalayout `opt` is told: which integer widths it takes as native (`n32`). The module keeps its own.
    pub native: Option<String>,
}

impl Mid {
    /// `LLRM_LLVM_MID` (`O2`, `Os`), `LLRM_LLVM_OPT`, `LLRM_LLVM_TRIPLE`, `LLRM_LLVM_CPU`, `LLRM_LLVM_FLAGS`.
    pub fn from_env(inline_limit: i64) -> Option<Self> {
        let level = std::env::var("LLRM_LLVM_MID").ok().filter(|one| !one.is_empty())?;
        let var = |name: &str, default: &str| std::env::var(name).unwrap_or_else(|_| default.to_owned());
        Some(Self {
            level,
            opt: var("LLRM_LLVM_OPT", "/usr/lib/llvm-20/bin/opt"),
            triple: var("LLRM_LLVM_TRIPLE", "i686-unknown-linux-gnu"),
            cpu: var("LLRM_LLVM_CPU", "i486"),
            inline_limit,
            native: std::env::var("LLRM_LLVM_NATIVE").ok().filter(|one| !one.is_empty()),
            extra: var("LLRM_LLVM_FLAGS", "-vectorize-loops=false -vectorize-slp=false").split_whitespace().map(str::to_owned).collect(),
        })
    }
}

/// Flags our dialect has that LLVM's attribute list lacks: sent as string attributes, which `opt` keeps.
const CUSTOM_FLAGS: [&str; 4] = ["nearcode", "noretain", "releases", "threeway"];

/// Each module of `program` through `opt`; `dump` gets `llvm-in-N.ll` and `llvm-out-N.ll`. What the program's
/// exports name (an entry the runtime calls, what the object keeps) is external to `opt` and internal again after.
pub fn run(program: &mut Program, mid: &Mid, dump: Option<&Path>) -> Result<(), String> {
    for at in 0..program.modules.len() {
        let exports = program.exports.clone();
        let module = &mut program.modules[at];
        let original: Vec<Linkage> = module.globals.iter().map(|global| global.linkage).collect();
        let mut shown = module.clone();
        for global in &mut shown.globals {
            if matches!(global.linkage, Linkage::Internal | Linkage::Private) && exports.exported(global) {
                global.linkage = Linkage::External;
            }
        }
        let text = exported(&shown, mid);
        if let Some(directory) = dump {
            std::fs::write(directory.join(format!("llvm-in-{at}.ll")), &text).map_err(|error| error.to_string())?;
        }
        let datalayout = module.datalayout.clone();
        let optimized = optimized(&text, mid)?;
        if let Some(directory) = dump {
            std::fs::write(directory.join(format!("llvm-out-{at}.ll")), &optimized).map_err(|error| error.to_string())?;
        }
        let mut imported = imported(&optimized)?;
        imported.datalayout = datalayout;
        // The globals `opt` left in place keep the linkage they had.
        let names: std::collections::HashMap<&str, Linkage> = module.globals.iter().zip(&original).filter_map(|(global, linkage)| Some((global.name.as_deref()?, *linkage))).collect();
        for global in &mut imported.globals {
            if let Some(linkage) = global.name.as_deref().and_then(|name| names.get(name)).filter(|linkage| matches!(linkage, Linkage::Internal | Linkage::Private)) {
                if exports.exported(&GlobalValue { linkage: *linkage, ..global.clone() }) {
                    global.linkage = *linkage;
                }
            }
        }
        *module = imported;
    }
    Ok(())
}

/// `module` as text `opt` takes: our conventions as numbers, a triple, and no inlining where the options say so.
pub fn exported(module: &Module, mid: &Mid) -> String {
    let mut text = llrm_mir::print::module(module);
    for (name, number) in CONVENTIONS.iter().filter(|(name, _)| *name == "sysvcc" || *name == "watcallcc") {
        text = replace_word(&text, name, &format!("cc {number}"));
    }
    for flag in CUSTOM_FLAGS {
        text = replace_word(&text, flag, &format!("\"llrm.flag.{flag}\""));
    }
    let mut out = String::new();
    for line in text.lines() {
        let mut line = line.to_owned();
        if let (Some(native), true) = (&mid.native, line.starts_with("target datalayout")) {
            let at = line.find("-n").map(|at| at + 1);
            let end = at.and_then(|at| line[at..].find('-').map(|end| at + end)).unwrap_or(line.len() - 1);
            if let Some(at) = at {
                line.replace_range(at..end, native);
            }
        }
        if line.starts_with("define ") {
            let mut added = String::new();
            // -fno-inline-functions: nothing inlines but the one call of a static, as ours (`last`).
            if mid.inline_limit == 0 && !(line.contains(" internal ") && called_once(&text, &line)) {
                added.push_str(" noinline");
            }
            // clang marks every function of a -Os (-Oz) build; `opt` alone does not, and its unroller and inliner read it.
            match mid.level.as_str() {
                "Os" => added.push_str(" optsize"),
                "Oz" => added.push_str(" optsize minsize"),
                _ => {}
            }
            if let Some(brace) = line.rfind(" {") {
                line.insert_str(brace, &added);
            }
        }
        out.push_str(&line);
        out.push('\n');
        if line.starts_with("target datalayout") {
            out.push_str(&format!("target triple = \"{}\"\n", mid.triple));
        }
    }
    out
}

fn optimized(text: &str, mid: &Mid) -> Result<String, String> {
    let mut command = Command::new(&mid.opt);
    command.arg(format!("-passes=default<{}>", mid.level)).arg("-S").arg(format!("-mcpu={}", mid.cpu)).args(&mid.extra);
    if mid.inline_limit > 0 {
        command.arg(format!("-inline-threshold={}", mid.inline_limit));
    }
    let mut child = command.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(|error| format!("{}: {error}", mid.opt))?;
    let mut stdin = child.stdin.take().expect("piped");
    let input = text.to_owned();
    let writer = std::thread::spawn(move || {
        use std::io::Write;
        stdin.write_all(input.as_bytes())
    });
    let done = child.wait_with_output().map_err(|error| error.to_string())?;
    let _ = writer.join();
    if !done.status.success() {
        return Err(format!("opt: {}", String::from_utf8_lossy(&done.stderr)));
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

/// `opt`'s output as a module: what the parser refuses by name is rewritten or dropped first.
pub fn imported(text: &str) -> Result<Module, String> {
    let mut kept = String::new();
    for line in text.lines() {
        if line.starts_with("; ModuleID") || line.starts_with("source_filename") || line.starts_with("target triple") {
            continue;
        }
        // The inliner's marker of a noalias scope it copied: it states nothing the `!noalias` attachments do not.
        if line.contains("@llvm.experimental.noalias.scope.decl(") {
            continue;
        }
        // What `saturating` expands, no longer called.
        if line.starts_with("declare ") && EXPANDED.iter().any(|name| line.contains(&format!("@llvm.{name}."))) {
            continue;
        }
        match expanded(&line.replace("= tail call ", "= call ")) {
            Some(lines) => lines.iter().for_each(|one| {
                kept.push_str(one);
                kept.push('\n');
            }),
            None => {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    let mut kept = replace_word(&replace_word(&kept, "dso_local", ""), "undef", "poison");
    for flag in CUSTOM_FLAGS {
        kept = kept.replace(&format!("\"llrm.flag.{flag}\""), flag);
    }
    if std::env::var_os("LLRM_LLVM_KEEPTAIL").is_none() {
        // `tail` is a hint LLVM adds to any call that cannot see the caller's frame; the backend here reads it as a promise to jump.
        kept = kept.replace(" tail call ", " call ").replace("= tail call ", "= call ");
    }
    let module = llrm_mir::parse::module(&kept).map_err(|error| format!("import: {error}"))?;
    let problems = llrm_mir::verify::verify(&module);
    if let Some(first) = problems.first() {
        return Err(format!("import verify: {first} ({} problems)", problems.len()));
    }
    Ok(module)
}

/// Every whole-word `word` in `text` as `with`.
fn replace_word(text: &str, word: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    let word_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.' || byte == b'$' || byte == b'%' || byte == b'@';
    while let Some(at) = rest.find(word) {
        let before = rest[..at].bytes().next_back();
        let after = rest[at + word.len()..].bytes().next();
        out.push_str(&rest[..at]);
        if before.is_some_and(word_byte) || after.is_some_and(word_byte) {
            out.push_str(word);
        } else {
            out.push_str(with);
        }
        rest = &rest[at + word.len()..];
    }
    out.push_str(rest);
    out
}

/// Whether the function `define_line` defines is named once besides its definition: one direct call, no address taken.
fn called_once(text: &str, define_line: &str) -> bool {
    let Some(at) = define_line.find('@') else { return false };
    let name: String = define_line[at + 1..].chars().take_while(|one| one.is_ascii_alphanumeric() || matches!(one, '_' | '.' | '$')).collect();
    let needle = format!("@{name}");
    let mut seen = 0;
    let mut rest = text;
    while let Some(found) = rest.find(&needle) {
        let after = rest[found + needle.len()..].bytes().next();
        if !after.is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$')) {
            seen += 1;
        }
        rest = &rest[found + needle.len()..];
    }
    seen == 2
}

/// The intrinsics `expanded` writes as plain instructions: MIR has no saturating arithmetic, and isel of m16 has no
/// `umax` (`opt` makes them of any compare and select).
const EXPANDED: [&str; 6] = ["usub.sat", "uadd.sat", "smin", "smax", "umin", "umax"];

/// `%r = call iN @llvm.usub.sat.iN(iN a, iN b)` (or `uadd.sat`, `smin`, `smax`, `umin`, `umax`) as the compare and
/// select it means.
fn expanded(line: &str) -> Option<Vec<String>> {
    let (result, call) = line.trim().split_once(" = call ")?;
    let (before, rest) = call.split_once(" @llvm.")?;
    // The type is the last word before the callee; return attributes (`range(i16 8, -3)`, `noundef`) come ahead of it.
    let ty = before.rsplit(' ').next()?;
    let kind = EXPANDED.iter().find(|name| rest.starts_with(&format!("{name}.")))?;
    let (_, arguments) = rest.split_once('(')?;
    let arguments = arguments.strip_suffix(')')?;
    let value = |one: &str| one.trim().split_once(' ').map(|(_, value)| value.to_owned());
    let (a, b) = arguments.split_once(',')?;
    let (a, b) = (value(a)?, value(b)?);
    let name = result.trim_start_matches('%');
    let pick = |predicate: &str| {
        vec![
            format!("  %{name}.pick = icmp {predicate} {ty} {a}, {b}"),
            format!("  {result} = select i1 %{name}.pick, {ty} {a}, {ty} {b}"),
        ]
    };
    Some(match *kind {
        "smin" => pick("slt"),
        "smax" => pick("sgt"),
        "umin" => pick("ult"),
        "umax" => pick("ugt"),
        "usub.sat" => vec![
            format!("  %{name}.sat.c = icmp ugt {ty} {a}, {b}"),
            format!("  %{name}.sat.s = sub {ty} {a}, {b}"),
            format!("  {result} = select i1 %{name}.sat.c, {ty} %{name}.sat.s, {ty} 0"),
        ],
        _ => vec![
            format!("  %{name}.sat.s = add {ty} {a}, {b}"),
            format!("  %{name}.sat.c = icmp ult {ty} %{name}.sat.s, {a}"),
            format!("  {result} = select i1 %{name}.sat.c, {ty} -1, {ty} %{name}.sat.s"),
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "target datalayout = \"e-p:16:16-p1:32:16:16:16-i32:16-i64:16\"\n\n";

    fn mid() -> Mid {
        Mid { level: "O2".into(), opt: String::new(), triple: "i386-unknown-linux-gnu".into(), cpu: "i386".into(), inline_limit: 225, extra: Vec::new(), native: None }
    }

    /// `opt` makes `usub.sat` of Nib's clamped subtraction and MIR had no such intrinsic: 20 of the gate's Nib programs
    /// were refused at import ("@llvm.usub.sat.i16: an intrinsic MIR does not have").
    #[test]
    fn a_saturating_subtraction_is_read_as_the_select_it_means() {
        let text = format!("{HEAD}declare i16 @llvm.usub.sat.i16(i16, i16)\n\ndefine i16 @f(i16 %a, i16 %b) {{\nb0:\n  %r = call i16 @llvm.usub.sat.i16(i16 %a, i16 %b)\n  ret i16 %r\n}}\n");
        let module = imported(&text).unwrap_or_else(|error| panic!("{error}"));
        let printed = llrm_mir::print::module(&module);
        assert!(printed.contains("select") && !printed.contains("@llvm.usub"), "{printed}");
    }

    /// `opt` writes a compare and select as `llvm.umax`, which the m16 selector has no pattern for: Nib's shellsort
    /// stopped at "@main: @llvm.umax.i16" after the import.
    #[test]
    fn a_min_or_max_is_read_as_the_compare_and_select_it_means() {
        let text = format!("{HEAD}declare i16 @llvm.umax.i16(i16, i16)\n\ndefine i16 @f(i16 %a, i16 %b) {{\nb0:\n  %r = tail call range(i16 8, -3) i16 @llvm.umax.i16(i16 %a, i16 %b)\n  ret i16 %r\n}}\n");
        let printed = llrm_mir::print::module(&imported(&text).unwrap_or_else(|error| panic!("{error}")));
        assert!(printed.contains("icmp ugt") && printed.contains("select") && !printed.contains("llvm.umax"), "{printed}");
    }

    /// The inliner leaves `llvm.experimental.noalias.scope.decl(metadata !N)`, a metadata operand the parser refuses
    /// ("expected a constant, found MetadataId") and MIR has nothing to say with.
    #[test]
    fn the_inliners_noalias_scope_marker_is_dropped() {
        let text = format!("{HEAD}declare void @llvm.experimental.noalias.scope.decl(metadata)\n\ndefine void @f() {{\nb0:\n  tail call void @llvm.experimental.noalias.scope.decl(metadata !1)\n  ret void\n}}\n\n!0 = distinct !{{!0}}\n!1 = !{{!2}}\n!2 = distinct !{{!2, !0}}\n");
        let module = imported(&text).unwrap_or_else(|error| panic!("{error}"));
        assert!(!llrm_mir::print::module(&module).contains("scope.decl\x28"), "kept");
    }

    /// Our conventions and flags LLVM has no word for go out as `cc N` and string attributes, and come back as written:
    /// `nearcode` and `watcallcc` made `opt` refuse every Nib and BASIC module.
    #[test]
    fn our_words_leave_as_llvms_and_return_as_ours() {
        let text = format!("{HEAD}define watcallcc void @f(ptr noretain %p) nearcode {{\nb0:\n  ret void\n}}\n");
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}"));
        let out = exported(&module, &mid());
        assert!(out.contains("cc 1002") && out.contains("\"llrm.flag.nearcode\"") && !out.contains("watcallcc"), "{out}");
        let back = imported(&out).unwrap_or_else(|error| panic!("{error}\n{out}"));
        assert_eq!(llrm_mir::print::module(&back), llrm_mir::print::module(&module));
    }
}
