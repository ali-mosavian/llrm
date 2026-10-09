//! The BASIC calling-convention matrix: llrm-qb's lowering of
//! tests/fixtures/callconv/bas against BC's (VBDOS, PDS 7.1, QB 4.5), both
//! read by `testing::boundary`. See tests/fixtures/callconv/readme.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use llrm_core::testing::boundary::{self, Byte, Procedure};

use super::compile as qb_compile;
use super::driver as qb_driver;

const DIALECTS: [&str; 3] = ["vbdos", "pds71", "qb45"];
/// Callee modules, then callers. CUE and CVE are PDS 7.1's and VBDOS's only.
const MODULES: [&str; 6] = ["CE", "CES", "CUE", "CVE", "CR", "CRS"];

/// Where llrm-qb and BC are known to differ: dialect and fact. Each fix
/// removes its lines; a line that no longer differs fails the test.
const KNOWN: &str = include_str!("../../../../tests/fixtures/callconv/bas/known.txt");

fn fixtures() -> PathBuf {
    Path::new(env!("LLRM_ROOT")).join("tests/fixtures/callconv/bas")
}

/// A byte as a fact states it: `-` where BC leaves whatever was there.
fn token(
    byte: &Byte,
    reference: bool,
    stack: bool,
) -> String {
    let junk = match byte {
        Byte::Unknown => true,
        Byte::Entry(register, _) => !matches!(*register, "ss" | "ds"),
        Byte::Returned(..) | Byte::Written(..) => stack,
        Byte::Sign(inner) => matches!(**inner, Byte::Unknown | Byte::Returned(..)),
        // Read through a pointer BC made from its own temporaries.
        Byte::Pointed(pointer, _) => pointer.contains(&Byte::Unknown),
        _ => false,
    };
    match byte {
        _ if reference && junk => "-".to_owned(),
        // An address, of the frame, of data or from a pointer: which one is
        // BC's or llrm's to choose.
        Byte::Address(_, _, part) => format!("&.{part}"),
        Byte::Entry("ss" | "ds", part) => format!("dgroup.{part}"),
        other => named(other),
    }
}

/// A byte as the instrument writes it, each procedure named without its type
/// suffix, as llrm's listing names it.
fn named(byte: &Byte) -> String {
    let all = |bytes: &[Byte]| bytes.iter().map(named).collect::<Vec<_>>().join(" ");
    match byte {
        Byte::Returned(call, register, at) => format!("{}:{register}.{at}", plain(call)),
        Byte::Written(call, at) => format!("{}:*{at}", plain(call)),
        Byte::Pointed(pointer, at) => format!("*({}){at:+}", all(pointer)),
        Byte::Sign(inner) => format!("sign({})", named(inner)),
        Byte::Converted(source, width, at) => format!("({}):f{}[{at}]", all(source), u32::from(*width) * 8),
        other => other.to_string(),
    }
}

fn tokens(
    bytes: &[Byte],
    reference: bool,
    stack: bool,
) -> Vec<String> {
    bytes.iter().map(|one| token(one, reference, stack)).collect()
}

/// A procedure's name without its type suffix, which llrm's listing drops.
fn plain(name: &str) -> String {
    name.to_uppercase().trim_end_matches(['%', '&', '!', '#', '$', '@']).to_owned()
}

/// Where a FUNCTION leaves its result, by its type's suffix.
fn result(
    suffix: char,
    procedure: &Procedure,
    reference: bool,
) -> Vec<String> {
    match Some(suffix) {
        Some('%' | '$') => [vec!["ax".to_owned()], tokens(&procedure.register("ax"), reference, false)].concat(),
        Some('&') => [vec!["dxax".to_owned()], tokens(&procedure.register("dxax"), reference, false)].concat(),
        _ => {
            let through = procedure
                .through
                .iter()
                .map(|(pointer, bytes)| format!("through {pointer}: {}", tokens(bytes, reference, false).join(" ")))
                .collect::<Vec<_>>();
            [through, vec!["ax".to_owned()], tokens(&procedure.register("ax"), reference, false)].concat()
        }
    }
}

fn kept(procedure: &Procedure) -> Vec<String> {
    let entry = |register: &'static str| (0..2).map(|at| Byte::Entry(register, at)).collect::<Vec<_>>();
    let mut kept: Vec<String> = [("si", "esi"), ("di", "edi"), ("bp", "ebp"), ("ds", "ds"), ("ss", "ss")]
        .iter()
        .filter(|(name, full)| procedure.register(name) == entry(full))
        .map(|(name, _)| name.to_string())
        .collect();
    if procedure.register("sp")
        == (0..2).map(|at| Byte::Address(Box::new(boundary::Base::Stack), 0, at)).collect::<Vec<_>>()
    {
        kept.push("sp".to_owned());
    }
    if procedure.forward {
        kept.push("df".to_owned());
    }
    kept
}

/// A branch cuts a run short; only what the harness calls straight through is
/// compared.
const PROBE: [&str; 5] = ["TRASH", "ARM", "VERIFY", "ARMX", "VERIFYX"];

/// Each FUNCTION's type suffix, by its plain name, as BC's listing names it.
fn suffixes(module: &BTreeMap<String, Procedure>) -> BTreeMap<String, char> {
    module
        .keys()
        .filter_map(|name| Some((plain(name), name.chars().last().filter(|one| "%&!#$@".contains(*one))?)))
        .collect()
}

fn facts(
    module: &BTreeMap<String, Procedure>,
    suffixes: &BTreeMap<String, char>,
    reference: bool,
) -> Vec<(String, Vec<String>, usize)> {
    let mut facts = Vec::new();
    for (full, procedure) in module.iter().filter(|(_, one)| !one.cut) {
        let name = &plain(full);
        facts.push((
            format!("{name} return"),
            vec![if procedure.far { "far" } else { "near" }.to_owned(), format!("pops {}", procedure.popped)],
            procedure.line,
        ));
        facts.push((format!("{name} keeps"), kept(procedure), procedure.line));
        if let Some(&suffix) = suffixes.get(name) {
            facts.push((format!("{name} result"), result(suffix, procedure, reference), procedure.line));
        }
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for call in procedure
            .calls
            .iter()
            .filter(|call| !PROBE.contains(&call.target.to_uppercase().as_str()) && !call.target.starts_with("b$"))
        {
            let target = plain(&call.target);
            let count = seen.entry(target.clone()).or_default();
            *count += 1;
            facts.push((
                format!("{name} call {target} #{count} stack"),
                tokens(&call.stack, reference, true),
                procedure.line,
            ));
            facts.push((
                format!("{name} call {target} #{count} cleanup"),
                vec![if call.far { "far" } else { "near" }.to_owned(), format!("pops {}", call.popped)],
                procedure.line,
            ));
        }
    }
    facts
}

fn bc(
    dialect: &str,
    module: &str,
) -> Option<BTreeMap<String, Procedure>> {
    let text = std::fs::read_to_string(fixtures().join(format!("bc/{dialect}/{module}.LST"))).ok()?;
    Some(boundary::procedures(&text).into_iter().map(|(name, one)| (name.to_uppercase(), one)).collect())
}

/// `module` as llrm-qb lowers it for `dialect`. It refuses CURRENCY, so it
/// sees the empty qb45/ CUR*.BI; the reference's CURRENCY facts are known
/// absent.
fn llrm(
    dialect: &str,
    module: &str,
) -> Result<BTreeMap<String, Procedure>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(fixtures()).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.is_file() {
            std::fs::copy(&path, directory.path().join(path.file_name().expect("a name")))
                .map_err(|error| error.to_string())?;
        }
    }
    let stubs: &[&str] = if dialect == "qb45" {
        &["CUR.BI", "CURCALL.BI", "CURCHK.BI", "V71.BI", "V71CALL.BI", "V71CHK.BI"]
    } else {
        &["CUR.BI", "CURCALL.BI", "CURCHK.BI"]
    };
    for stub in stubs {
        std::fs::copy(fixtures().join("qb45").join(stub), directory.path().join(stub))
            .map_err(|error| error.to_string())?;
    }
    let source = directory.path().join(format!("{module}.BAS"));
    let program = qb_driver::parsed(
        &source,
        &qb_driver::Frontend { runtime_frames: true, ..qb_driver::Frontend::new(dialect, dialect) },
        None,
    )
    .map_err(|error| error.0)?;
    let codegen = llrm_driver::m16_options(llrm_x86_m16::machine::BASIC.clone());
    let assembled = qb_compile::assembled(&program, None, &codegen).map_err(|error| error.to_string())?;
    let text = llrm_core::driver::basic::text(&assembled)?;
    Ok(boundary::procedures(&text).into_iter().map(|(name, one)| (name.to_uppercase(), one)).collect())
}

fn reference_table() -> String {
    let mut out = String::new();
    for dialect in DIALECTS {
        for module in MODULES {
            let Some(procedures) = bc(dialect, module) else { continue };
            for (name, value, line) in facts(&procedures, &suffixes(&procedures), true) {
                out += &format!("{dialect:<5} {name:<36} {:<40} bc/{dialect}/{module}.LST:{line}\n", value.join(" "));
            }
        }
    }
    out
}

/// The table is what BC's listings say, so it cannot drift from them.
#[test]
fn test_the_reference_table_is_what_bc_listed() {
    let path = fixtures().join("reference.txt");
    let derived = reference_table();
    if std::env::var_os("LLRM_BLESS").is_some() {
        std::fs::write(&path, &derived).unwrap();
    }
    assert!(
        std::fs::read_to_string(&path).unwrap() == derived,
        "tests/fixtures/callconv/bas/reference.txt is stale; LLRM_BLESS=1 rewrites it"
    );
}

/// Every fact BC shows at a call boundary, llrm-qb shows alike, but for the
/// mismatches known.txt names.
#[test]
fn test_llrm_lowers_each_call_boundary_as_bc_does() {
    let known: Vec<&str> =
        KNOWN.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    let (mut differ, mut report) = (Vec::new(), Vec::new());
    for dialect in DIALECTS {
        for module in MODULES {
            let Some(reference) = bc(dialect, module) else { continue };
            let named = suffixes(&reference);
            let got = match llrm(dialect, module) {
                Ok(one) => facts(&one, &named, false)
                    .into_iter()
                    .map(|(name, value, _)| (name, value))
                    .collect::<BTreeMap<_, _>>(),
                Err(error) => {
                    differ.push(format!("{dialect} {module} refused"));
                    report.push(format!("{dialect} {module} refused: {error}"));
                    continue;
                }
            };
            for (name, want, _) in facts(&reference, &named, true) {
                let got = got.get(&name).cloned().unwrap_or_else(|| vec!["absent".to_owned()]);
                let agrees =
                    want.len() == got.len() && want.iter().zip(&got).all(|(want, got)| want == "-" || want == got);
                if !agrees {
                    let key = format!("{dialect} {name}");
                    report.push(format!("{key}\n    bc   {}\n    llrm {}", want.join(" "), got.join(" ")));
                    differ.push(key);
                }
            }
        }
    }
    let unexpected: Vec<&String> = differ.iter().filter(|one| !known.contains(&one.as_str())).collect();
    let fixed: Vec<&&str> = known.iter().filter(|one| !differ.contains(&one.to_string())).collect();
    assert!(
        unexpected.is_empty() && fixed.is_empty(),
        "differ, not known:\n{}\n\nknown, no longer differ:\n{:?}\n\nall:\n{}",
        unexpected.iter().map(|one| one.as_str()).collect::<Vec<_>>().join("\n"),
        fixed,
        report.join("\n")
    );
}
