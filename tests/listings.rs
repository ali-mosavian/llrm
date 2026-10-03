//! A program written alike in BASIC, C and Nib compiles to the same loops:
//! each frontend states what it knows, and the shared optimizer and backend
//! do the rest. Loops are compared modulo register names, labels, symbols
//! and frame offsets.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const REGISTERS: [&str; 8] = ["ax", "bx", "cx", "dx", "si", "di", "bp", "sp"];

/// `register`'s family and width in bytes: `eax` is (ax, 4).
fn register(word: &str) -> Option<(&'static str, u8)> {
    let family = |name: &str| REGISTERS.iter().copied().find(|one| *one == name);
    if let Some(rest) = word.strip_prefix('e') {
        return family(rest).map(|one| (one, 4));
    }
    if let Some(one) = family(word) {
        return Some((one, 2));
    }
    let (first, last) = word.split_at(word.len().min(1));
    match last {
        "l" | "h" if word.len() == 2 => family(&format!("{first}x")).map(|one| (one, 1)),
        _ => None,
    }
}

/// Words that keep their spelling: mnemonics are the first word of a line.
const KEPT: [&str; 10] = ["byte", "word", "dword", "ptr", "offset", "seg", "far", "short", "es", "ds"];

/// `lines` with each register, label, symbol and frame offset renamed by
/// first appearance; constants kept.
fn normalized(lines: &[&str]) -> Vec<String> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let rename = |kind: &str, key: String, names: &mut BTreeMap<String, String>| {
        let count = names.keys().filter(|one| one.starts_with(kind)).count();
        names.entry(format!("{kind}{key}")).or_insert_with(|| format!("{kind}{count}")).clone()
    };
    let token = regex::Regex::new(r"[A-Za-z_$@?][A-Za-z0-9_$@?&%]*|\[bp[+-]\d+\]|[^A-Za-z_$@?\[]+|\[").unwrap();
    lines
        .iter()
        .map(|line| {
            let line = line.trim();
            let (mnemonic, rest) = line.split_once(' ').unwrap_or((line, ""));
            let mut out = String::new();
            if let Some(label) = mnemonic.strip_suffix(':') {
                return format!("{}:", rename("L", label.to_owned(), &mut names));
            }
            out.push_str(mnemonic);
            out.push(' ');
            for one in token.find_iter(rest).map(|one| one.as_str()) {
                if one.starts_with("[bp") {
                    out.push_str(&format!("[{}]", rename("F", one.to_owned(), &mut names)));
                } else if let Some((family, width)) = register(one) {
                    let name = rename("R", family.to_owned(), &mut names);
                    out.push_str(&format!("{name}.{width}"));
                } else if one.chars().next().is_some_and(|first| first.is_ascii_alphabetic() || "_$@?".contains(first)) && !KEPT.contains(&one) {
                    out.push_str(&rename(if one.starts_with('L') && one.contains('_') { "L" } else { "S" }, one.to_owned(), &mut names));
                } else {
                    out.push_str(one);
                }
            }
            out.trim_end().to_owned()
        })
        .collect()
}

/// The listing of the procedure `name` in `asm`.
fn procedure<'a>(asm: &'a str, name: &str) -> Vec<&'a str> {
    let start = format!("{name} proc");
    let mut lines = asm.lines().skip_while(|line| !line.starts_with(&start));
    lines.next().unwrap_or_else(|| panic!("no {start} in the listing"));
    lines.take_while(|line| !line.ends_with(" endp")).collect()
}

/// Each loop of `body`, outermost first: a label and every line up to the
/// last jump back to it.
fn loops<'a>(body: &[&'a str]) -> Vec<Vec<&'a str>> {
    let mut out = Vec::new();
    for (at, line) in body.iter().enumerate() {
        let Some(label) = line.strip_suffix(':') else { continue };
        let back = body.iter().rposition(|one| one.trim().starts_with('j') && one.trim().ends_with(&format!(" {label}")));
        if let Some(end) = back.filter(|&end| end > at) {
            out.push(body[at..=end].to_vec());
        }
    }
    out
}

fn compiled(tool: &str, source: &Path, arguments: &[&str]) -> String {
    let bin = Path::new(env!("CARGO_BIN_EXE_llrm-qb")).parent().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let out = scratch.path().join("out.asm");
    let done = Command::new(bin.join(tool)).arg(source).args(arguments).arg("-S").arg("-o").arg(&out).output().unwrap();
    assert!(done.status.success(), "{tool} {}: {}", source.display(), String::from_utf8_lossy(&done.stderr));
    std::fs::read_to_string(out).unwrap()
}

/// Each loop of `stem`'s kernel in BASIC, C and Nib, normalized.
fn kernels(dir: &Path, stem: &str, basic: &str) -> [(&'static str, Vec<Vec<String>>); 3] {
    let source = |extension: &str| dir.join(format!("{stem}.{extension}"));
    let cpu = ["-O2", "--cpu", "486"];
    let bas = compiled("llrm-qb", &source("bas"), &[&cpu[..], &["--dialect", "pds71", "--runtime", "pds71", "--huge-arrays"]].concat());
    let c = compiled("llrm-c", &source("c"), &[&cpu[..], &["-fno-inline-functions"]].concat());
    let nib = compiled("llrm-nib", &source("nib"), &cpu);
    let of = |asm: &str, name: &str| loops(&procedure(asm, name)).iter().map(|one| normalized(one)).collect();
    let kernel = format!("_bench_{stem}");
    [("bas", of(&bas, basic)), ("c", of(&c, &kernel)), ("nib", of(&nib, &kernel))]
}

/// Huge arrays: QB called B$HARY for every element, C redid the 32-bit
/// offset-to-segment arithmetic twice per element, and Nib had none (#362).
/// Every loop of each program is the same listing in all three.
#[test]
fn test_huge_array_loops_are_the_same_in_basic_c_and_nib() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut programs: Vec<(PathBuf, String, String)> = vec![(root.join("bench/huge"), "huge".into(), "BENCHHUGE".into())];
    let corpus = root.join("tests/run/huge");
    let mut stems: Vec<String> = std::fs::read_dir(&corpus).unwrap().flatten().filter_map(|one| {
        let path = one.path();
        (path.extension()? == "nib").then(|| path.file_stem().unwrap().to_string_lossy().into_owned())
    }).collect();
    stems.sort();
    assert!(stems.len() >= 5, "premise: the corpus is found: {stems:?}");
    programs.extend(stems.into_iter().map(|stem| (corpus.clone(), format!("BENCH{}", stem.to_uppercase()), stem)).map(|(dir, basic, stem)| (dir, stem, basic)));
    let mut differ = Vec::new();
    for (dir, stem, basic) in &programs {
        let [(_, bas), (_, c), (_, nib)] = kernels(dir, stem, basic);
        assert!(!c.is_empty(), "premise: {stem}.c has loops");
        if bas != c || nib != c {
            let show = |loops: &Vec<Vec<String>>| loops.iter().map(|one| one.join("\n")).collect::<Vec<_>>().join("\n--\n");
            differ.push(format!("== {stem}\n-- bas\n{}\n-- c\n{}\n-- nib\n{}", show(&bas), show(&c), show(&nib)));
        }
    }
    assert!(differ.is_empty(), "{} of {} differ:\n{}", differ.len(), programs.len(), differ.join("\n"));
}
