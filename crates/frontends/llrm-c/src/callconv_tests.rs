//! The C calling-convention matrix: llrm's lowering of
//! tests/fixtures/callconv/c against BCC 3.1's, both read by
//! `testing::boundary`. See tests/fixtures/callconv/readme.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use llrm_core::testing::boundary::{self, Byte, Procedure};

const CONVENTIONS: [&str; 4] = ["cf", "cn", "pf", "pn"];
const MODULES: [(&str, &str); 2] = [("callee", "caller"), ("aggee", "agger")];

/// Where llrm and BCC are known to differ: convention, module, fact. Each
/// fix removes its lines; a line that no longer differs fails the test.
const KNOWN: &str = include_str!("../../../../tests/fixtures/callconv/c/known.txt");

fn fixtures() -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join("tests/fixtures/callconv/c")
}

/// One observable fact: its name, its bytes or words, and where BCC shows it.
#[derive(Clone, Debug)]
struct Fact {
    name: String,
    value: Vec<String>,
    line: usize,
}

/// A byte as a fact states it: `-` where BCC leaves whatever was there. In
/// a stack image, a register some earlier call returned is such a byte.
fn junk(byte: &Byte, stack: bool) -> bool {
    match byte {
        Byte::Unknown => true,
        // A register's value on entry is junk pushed, but an interrupt handler's parameter kept.
        Byte::Entry(register, _) => stack && !matches!(*register, "ss" | "ds"),
        Byte::Returned(..) => stack,
        Byte::Sign(inner) => junk(inner, stack),
        // A struct's padding, pushed with it.
        Byte::Global(name, at) => name.contains("in_") && *at as usize >= size(name),
        _ => false,
    }
}

fn token(byte: &Byte, reference: bool, stack: bool) -> String {
    match byte {
        _ if reference && junk(byte, stack) => "-".to_owned(),
        Byte::Address(base, _, part) => format!("&{}.{part}", match **base {
            boundary::Base::Stack | boundary::Base::Global(_) => "data",
            boundary::Base::Segment(one) => one,
            boundary::Base::Pointer(_) => "pointer",
        }),
        Byte::Entry("ss" | "ds", part) => format!("dgroup.{part}"),
        other => other.to_string(),
    }
}

fn tokens(bytes: &[Byte], reference: bool) -> Vec<String> {
    bytes.iter().map(|one| token(one, reference, false)).collect()
}

/// A stack image's bytes. The word a prototyped byte argument is pushed in
/// holds junk above it; only a variadic call promotes it.
fn stack_tokens(bytes: &[Byte], reference: bool, variadic: bool) -> Vec<String> {
    let mut tokens: Vec<String> = bytes.iter().map(|one| token(one, reference, true)).collect();
    for (at, one) in bytes.iter().enumerate() {
        if let (Byte::Global(name, 0), false, true) = (one, variadic, reference) {
            if size(name) == 1 && at + 1 < tokens.len() {
                tokens[at + 1] = "-".to_owned();
            }
        }
    }
    tokens
}

/// How many bytes of a global the cases define: its type's size.
fn size(global: &str) -> usize {
    let (prefix, case) = global.trim_start_matches('_').split_once('_').expect("a case global");
    match (prefix, case) {
        ("gh" | "gt" | "gva", "n") | ("gh" | "gt", _) | ("gi", "plain") => 2,
        ("gi", "regs") => 18,
        ("gw" | "ow", _) => 4,
        // p_mixed's a..h: signed char, long, double, int, float, far pointer, long double.
        ("gm", "a") => 1,
        ("gm", "d") => 2,
        ("gm", "b" | "e" | "f") => 4,
        ("gm", "c") => 8,
        ("gm", "h") => 10,
        ("gva", "sc" | "uc" | "ch") => 2,
        ("gva", "f") => 8,
        (_, "sc" | "uc" | "ch" | "s1") => 1,
        (_, "i" | "u" | "np" | "s2") => 2,
        (_, "sci" | "s3") => 3,
        (_, "l" | "ul" | "fp" | "hp" | "f" | "s4") => 4,
        (_, "s5") => 5,
        (_, "s7") => 7,
        (_, "d" | "s8") => 8,
        (_, "s9") => 9,
        (_, "ld") => 10,
        _ => panic!("no size for {global}"),
    }
}

/// Where a callee leaves the result it read from `_in_*`: the narrowest of
/// al, ax, dx:ax, st(0) and memory through a pointer argument that holds it.
fn result(procedure: &Procedure, reference: bool) -> Vec<String> {
    if let Some((pointer, bytes)) = procedure.through.iter().next() {
        return [vec![format!("through {pointer}")], tokens(bytes, reference), vec!["dxax".to_owned()], tokens(&procedure.register("dxax"), reference)].concat();
    }
    let from_input = |bytes: &[Byte]| !bytes.is_empty() && bytes.iter().all(|one| matches!(one, Byte::Global(name, _) if name.contains("in_")));
    if let Some(top) = procedure.top.as_ref().filter(|top| top.iter().any(|one| matches!(one, Byte::Global(..)))) {
        let held: Vec<Byte> = top.iter().take_while(|one| matches!(one, Byte::Global(..))).cloned().collect();
        return [vec!["st".to_owned()], tokens(&held, reference)].concat();
    }
    for name in ["al", "ax", "dxax"] {
        let bytes = procedure.register(name);
        if from_input(&bytes) {
            return [vec![name.to_owned()], tokens(&bytes, reference)].concat();
        }
    }
    vec!["none".to_owned()]
}

/// The registers a callee returns as it found them.
fn kept(procedure: &Procedure) -> Vec<String> {
    let mut kept: Vec<String> = ["si", "di", "bp", "sp", "ds", "ss"]
        .iter()
        .filter(|name| procedure.register(name) == (0..2).map(|at| if **name == "sp" { Byte::Address(Box::new(boundary::Base::Stack), 0, at) } else { Byte::Entry(boundary_name(name), at) }).collect::<Vec<_>>())
        .map(|name| name.to_string())
        .collect();
    if procedure.forward {
        kept.push("df".to_owned());
    }
    kept
}

fn boundary_name(name: &str) -> &'static str {
    match name {
        "si" => "esi",
        "di" => "edi",
        "bp" => "ebp",
        "ds" => "ds",
        "ss" => "ss",
        other => panic!("{other}"),
    }
}

/// Every fact a module pair shows, named alike for BCC and llrm.
fn facts(callee: &BTreeMap<String, Procedure>, caller: &BTreeMap<String, Procedure>, reference: bool) -> Vec<Fact> {
    let mut facts = Vec::new();
    let mut add = |name: String, value: Vec<String>, line: usize| facts.push(Fact { name, value, line });
    for (name, procedure) in callee.iter().filter(|(name, _)| !name.contains("call_")) {
        let by = if procedure.iret { "iret" } else if procedure.far { "far" } else { "near" };
        add(format!("{name} return"), vec![by.to_owned(), format!("pops {}", procedure.popped)], procedure.line);
        add(format!("{name} keeps"), kept(procedure), procedure.line);
        for (global, bytes) in procedure.globals.iter().filter(|(global, _)| global.starts_with("_g")) {
            add(format!("{name} {global}"), tokens(&bytes[..size(global).min(bytes.len())], reference), procedure.line);
        }
        if name.to_lowercase().starts_with("_r_") || name.starts_with("R_") {
            add(format!("{name} result"), result(procedure, reference), procedure.line);
        }
    }
    for procedure in caller.values() {
        let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
        for call in procedure.calls.iter().filter(|call| !["_arm", "_verify", "_trash"].contains(&call.target.as_str())) {
            let count = seen.entry(&call.target).or_default();
            *count += 1;
            let name = format!("call {} #{count}", call.target);
            add(format!("{name} stack"), stack_tokens(&call.stack, reference, call.target.contains("variadic")), procedure.line);
            add(format!("{name} cleanup"), vec![if call.far { "far" } else { "near" }.to_owned(), format!("pops {}", call.popped)], procedure.line);
        }
        for (global, bytes) in procedure.globals.iter().filter(|(global, _)| global.starts_with("_o")) {
            add(format!("keeps {global}"), tokens(&bytes[..size(global).min(bytes.len())], reference), procedure.line);
        }
    }
    facts
}

fn agrees(reference: &[String], llrm: &[String]) -> bool {
    reference.len() == llrm.len() && reference.iter().zip(llrm).all(|(want, got)| want == "-" || want == got)
}

fn llrm(file: &str) -> Result<BTreeMap<String, Procedure>, String> {
    let stream = crate::compile::recorded(&fixtures().join(format!("{file}.c")), &[], false, &[]).map_err(|error| error.0)?;
    let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
    let built = crate::compile::selected(&stream, file, None, &llrm_driver::m16_options(machine)).map_err(|error| format!("{error:?}"))?;
    Ok(boundary::procedures(&llrm_core::backend::masm::text(&built).map_err(|error| format!("{error:?}"))?))
}

fn bcc(file: &str) -> BTreeMap<String, Procedure> {
    boundary::procedures(&std::fs::read_to_string(fixtures().join(format!("bcc/{}.ASM", file.to_uppercase()))).unwrap())
}

/// The reference, as the table tests/fixtures/callconv/c/reference.txt records it.
fn reference_table() -> String {
    let mut out = String::new();
    for convention in CONVENTIONS {
        for (callee, caller) in MODULES {
            for fact in facts(&bcc(&format!("{convention}{callee}")), &bcc(&format!("{convention}{caller}")), true) {
                let file = if fact.name.starts_with("call ") || fact.name.starts_with("keeps _o") { caller } else { callee };
                out += &format!("{convention} {:<28} {:<40} bcc/{}{}.ASM:{}\n", fact.name, fact.value.join(" "), convention.to_uppercase(), file.to_uppercase(), fact.line);
            }
        }
    }
    out
}

/// The table is what BCC's listings say, so it cannot drift from them.
#[test]
fn test_the_reference_table_is_what_bcc_listed() {
    let path = fixtures().join("reference.txt");
    let derived = reference_table();
    if std::env::var_os("LLRM_BLESS").is_some() {
        std::fs::write(&path, &derived).unwrap();
    }
    assert!(std::fs::read_to_string(&path).unwrap() == derived, "tests/fixtures/callconv/c/reference.txt is stale; LLRM_BLESS=1 rewrites it");
}

/// BCC's own callees keep SI, DI, BP, SP, DS and SS and clear DF: the
/// instrument reads a Borland callee as one.
#[test]
fn test_bcc_callees_keep_what_the_convention_keeps() {
    for convention in CONVENTIONS {
        for (callee, _) in MODULES {
            for (name, procedure) in bcc(&format!("{convention}{callee}")) {
                assert_eq!(kept(&procedure), ["si", "di", "bp", "sp", "ds", "ss", "df"], "{convention} {name}");
            }
        }
    }
}

/// Every fact BCC shows at a call boundary, llrm shows alike, but for the
/// mismatches known.txt names.
// It records C through wccq, which only the toolchain feature builds.
#[cfg(feature = "toolchain")]
#[test]
fn test_llrm_lowers_each_call_boundary_as_bcc_does() {
    let known: Vec<&str> = KNOWN.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    let mut differ = Vec::new();
    let mut report = Vec::new();
    for convention in CONVENTIONS {
        for (callee, caller) in MODULES {
            let (callee_file, caller_file) = (format!("{convention}{callee}"), format!("{convention}{caller}"));
            let want = facts(&bcc(&callee_file), &bcc(&caller_file), true);
            let (llrm_callee, llrm_caller) = match (llrm(&callee_file), llrm(&caller_file)) {
                (Ok(one), Ok(other)) => (one, other),
                (one, other) => {
                    for (file, result) in [(&callee_file, one.err()), (&caller_file, other.err())] {
                        if let Some(error) = result {
                            differ.push(format!("{file} refused"));
                            report.push(format!("{file} refused: {error}"));
                        }
                    }
                    continue;
                }
            };
            let got: BTreeMap<String, Vec<String>> = facts(&llrm_callee, &llrm_caller, false).into_iter().map(|fact| (fact.name, fact.value)).collect();
            for fact in want {
                let got = got.get(&fact.name).cloned().unwrap_or_else(|| vec!["absent".to_owned()]);
                if !agrees(&fact.value, &got) {
                    let key = format!("{convention} {}", fact.name);
                    report.push(format!("{key}\n    bcc  {}\n    llrm {}", fact.value.join(" "), got.join(" ")));
                    differ.push(key);
                }
            }
        }
    }
    let unexpected: Vec<&String> = differ.iter().filter(|one| !known.contains(&one.as_str())).collect();
    let fixed: Vec<&&str> = known.iter().filter(|one| !differ.contains(&one.to_string())).collect();
    assert!(unexpected.is_empty() && fixed.is_empty(), "differ, not known:\n{}\n\nknown, no longer differ:\n{:?}\n\nall:\n{}", unexpected.iter().map(|one| one.as_str()).collect::<Vec<_>>().join("\n"), fixed, report.join("\n"));
}

/// `text` compiled by llrm, read by the instrument.
fn llrm_text(text: &str) -> BTreeMap<String, Procedure> {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("probe.c");
    std::fs::write(&source, text).unwrap();
    let stream = crate::compile::recorded(&source, &[], false, &[]).unwrap();
    let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
    let built = crate::compile::selected(&stream, "probe", None, &llrm_driver::m16_options(machine)).unwrap();
    boundary::procedures(&llrm_core::backend::masm::text(&built).unwrap())
}

/// A signed char passed through `...` is promoted to int: llrm zero-extended
/// it, so p_variadic read -2 as 254.
// It records C through wccq, which only the toolchain feature builds.
#[cfg(feature = "toolchain")]
#[test]
fn test_a_variadic_byte_is_promoted_by_its_signedness() {
    let built = llrm_text("signed char s; unsigned char u;\nvoid far v(int n, ...);\nvoid far f(void) { v(1, s, u); }\n");
    let stack = &built["_f"].calls[0].stack;
    let sign = Byte::Sign(Box::new(Byte::Global("_s".into(), 0)));
    assert_eq!(stack[2..], [Byte::Global("_s".into(), 0), sign, Byte::Global("_u".into(), 0), Byte::Const(0)]);
}

/// A variadic function taking a struct parameter's address got the address
/// of the struct's local copy, so STDARG.H's `va_start` stepped from there
/// into the frame; it is refused rather than miscompiled.
// It records C through wccq, which only the toolchain feature builds.
#[cfg(feature = "toolchain")]
#[test]
fn test_a_variadic_struct_parameter_address_is_refused() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("probe.c");
    std::fs::write(&source, "typedef struct { char b[3]; } S3;\nint g;\nvoid far v(S3 s, ...) { char *p = (char *)&s; g = p[4]; }\n").unwrap();
    let stream = crate::compile::recorded(&source, &[], false, &[]).unwrap();
    let machine = llrm_core::abi::machine::Machine { cpu: "386".to_owned(), ..llrm_x86_m16::machine::BUILT_IN.clone() };
    let built = crate::compile::selected(&stream, "probe", None, &llrm_driver::m16_options(machine));
    assert!(format!("{:?}", built.err()).contains("the address of a struct parameter"));
}
