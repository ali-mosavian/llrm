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

/// Instructions that write their first operand without reading it.
const DEFINING: [&str; 4] = ["mov", "movzx", "movsx", "lea"];

/// `lines` with each label, symbol and frame offset renamed by first
/// appearance, and each register by the value it holds: one written without
/// being read holds a new one. Constants are kept.
fn normalized(lines: &[&str]) -> Vec<String> {
    let mut names: BTreeMap<String, String> = BTreeMap::new();
    let mut versions: BTreeMap<&str, usize> = BTreeMap::new();
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
            if let Some(label) = mnemonic.strip_suffix(':') {
                return format!("{}:", rename("L", label.to_owned(), &mut names));
            }
            // A register destination written whole: `mov r, x`, or `sbb r, r` and its kin.
            let operands: Vec<&str> = rest.split(',').map(str::trim).collect();
            let idiom = ["sbb", "xor", "sub"].contains(&mnemonic) && operands.len() == 2 && operands[0] == operands[1];
            let written = register(operands[0]).filter(|_| DEFINING.contains(&mnemonic) || idiom);
            let tokens: Vec<&str> = token.find_iter(rest).map(|one| one.as_str()).collect();
            let mut out = vec![String::new(); tokens.len()];
            let mut name_of = |one: &str, at: usize, names: &mut BTreeMap<String, String>, versions: &BTreeMap<&str, usize>| {
                if one.starts_with("[bp") {
                    format!("[{}]", rename("F", one.to_owned(), names))
                } else if let Some((family, width)) = register(one) {
                    // `sbb r, r` reads nothing: every operand is the new value.
                    let new = written.is_some_and(|(of, _)| of == family) && (at == 0 || idiom);
                    let version = versions.get(family).copied().unwrap_or(0) + usize::from(new);
                    format!("{}.{width}", rename("R", format!("{family}#{version}"), names))
                } else if one.chars().next().is_some_and(|first| first.is_ascii_alphabetic() || "_$@?".contains(first)) && !KEPT.contains(&one) {
                    rename(if one.starts_with('L') && one.contains('_') { "L" } else { "S" }, one.to_owned(), names)
                } else {
                    one.to_owned()
                }
            };
            // Sources read the old value; the destination names the new one.
            for (at, one) in tokens.iter().enumerate().skip(1) {
                out[at] = name_of(one, at, &mut names, &versions);
            }
            if let Some(first) = tokens.first() {
                out[0] = name_of(first, 0, &mut names, &versions);
            }
            if let Some((family, _)) = written {
                *versions.entry(family).or_insert(0) += 1;
            }
            format!("{mnemonic} {}", out.concat()).trim_end().to_owned()
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
/// last jump back to it, and whether no other loop is inside it.
fn loops<'a>(body: &[&'a str]) -> Vec<(Vec<&'a str>, bool)> {
    let mut spans = Vec::new();
    for (at, line) in body.iter().enumerate() {
        let Some(label) = line.strip_suffix(':') else { continue };
        let back = body.iter().rposition(|one| one.trim().starts_with('j') && one.trim().ends_with(&format!(" {label}")));
        if let Some(end) = back.filter(|&end| end > at) {
            spans.push((at, end));
        }
    }
    spans.iter().map(|&(at, end)| (body[at..=end].to_vec(), !spans.iter().any(|&(one, other)| (one, other) != (at, end) && at <= one && other <= end))).collect()
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
fn kernels(dir: &Path, stem: &str, basic: &str) -> [(&'static str, Vec<(Vec<String>, bool)>); 3] {
    let source = |extension: &str| dir.join(format!("{stem}.{extension}"));
    let cpu = ["-O2", "--cpu", "486"];
    let bas = compiled("llrm-qb", &source("bas"), &[&cpu[..], &["--dialect", "pds71", "--runtime", "pds71", "--huge-arrays"]].concat());
    let c = compiled("llrm-c", &source("c"), &[&cpu[..], &["-fno-inline-functions"]].concat());
    let nib = compiled("llrm-nib", &source("nib"), &cpu);
    let of = |asm: &str, name: &str| loops(&procedure(asm, name)).iter().map(|(one, inner)| (normalized(one), *inner)).collect();
    let kernel = format!("_bench_{stem}");
    [("bas", of(&bas, basic)), ("c", of(&c, &kernel)), ("nib", of(&nib, &kernel))]
}

/// Huge arrays: QB called B$HARY for every element, C redid the 32-bit
/// offset-to-segment arithmetic twice per element, and Nib had none (#362).
/// Every loop of each program is the same listing in all three, and an
/// inner loop never carries into its selector: a walk past 64K paid the
/// carry every trip (copy1d's copy loop, 16 instructions).
#[test]
fn test_huge_array_loops_are_the_same_in_basic_c_and_nib() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut programs: Vec<(PathBuf, String, String)> = vec![(root.join("bench/huge"), "huge".into(), "BENCHHUGE".into())];
    let corpus = root.join("tests/run/huge");
    let mut stems: Vec<String> = std::fs::read_dir(&corpus).unwrap().flatten().filter_map(|one| {
        let path = one.path();
        // copyw and fillw are 16-bit words past 64K, which BASIC can only index as two columns (a subscript is at
        // most 32767): their loops differ by construction. tests/run holds all three and compares their output.
        let stem = path.file_stem()?.to_string_lossy().into_owned();
        (path.extension()? == "nib" && !["copyw", "fillw"].contains(&stem.as_str())).then_some(stem)
    }).collect();
    stems.sort();
    assert!(stems.len() >= 5, "premise: the corpus is found: {stems:?}");
    programs.extend(stems.into_iter().map(|stem| (corpus.clone(), format!("BENCH{}", stem.to_uppercase()), stem)).map(|(dir, basic, stem)| (dir, stem, basic)));
    let (mut differ, mut carried) = (Vec::new(), Vec::new());
    for (dir, stem, basic) in &programs {
        let languages = kernels(dir, stem, basic);
        let [(_, bas), (_, c), (_, nib)] = &languages;
        assert!(!c.is_empty(), "premise: {stem}.c has loops");
        if bas != c || nib != c {
            let show = |loops: &Vec<(Vec<String>, bool)>| loops.iter().map(|(one, _)| one.join("\n")).collect::<Vec<_>>().join("\n--\n");
            differ.push(format!("== {stem}\n-- bas\n{}\n-- c\n{}\n-- nib\n{}", show(bas), show(c), show(nib)));
        }
        // Inside a window nothing carries into a selector: no borrow mask.
        for (language, loops) in &languages {
            for (one, _) in loops.iter().filter(|(_, inner)| *inner) {
                if one.iter().any(|line| line.starts_with("sbb ") || line.ends_with(", 4096")) {
                    carried.push(format!("== {stem}.{language}\n{}", one.join("\n")));
                }
            }
        }
    }
    assert!(differ.is_empty(), "{} of {} differ:\n{}", differ.len(), programs.len(), differ.join("\n"));
    assert!(carried.is_empty(), "an inner loop carries:\n{}", carried.join("\n"));
}

/// Scroll's loop saved ES, set it to DS and restored it around each of its
/// four string ops (16 instructions a trip), and began each backward copy
/// with `sub si, 3` after the `mov si, K` (#493, #494). In all three
/// languages the loop holds only the string ops' own setup.
#[test]
fn test_scroll_loop_sets_no_segment_and_steps_no_start() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let languages = kernels(&root.join("bench/scroll"), "scroll", "BENCHSCROLL");
    for (language, loops) in &languages {
        let strings: Vec<_> = loops.iter().filter(|(one, _)| one.iter().any(|line| line.starts_with("rep "))).collect();
        assert_eq!(strings.len(), 1, "premise: {language} has one loop of string ops: {loops:?}");
        let body = &strings[0].0;
        assert_eq!(body.iter().filter(|line| line.starts_with("rep ")).count(), 4, "premise: {language}: {body:?}");
        let segment = body.iter().filter(|line| line.contains("es") && (line.starts_with("push") || line.starts_with("pop") || line.starts_with("mov es"))).count();
        let stepped = body.iter().filter(|line| line.starts_with("sub ") && line.ends_with(", 3")).count();
        assert_eq!((segment, stepped), (0, 0), "{language}:\n{}", body.join("\n"));
    }
}
