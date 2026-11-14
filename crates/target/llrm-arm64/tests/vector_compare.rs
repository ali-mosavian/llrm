//! A vector `icmp` was refused ("vector with no register class": no `<N x i1>`
//! type, no `VSETCC`). Each case loads two vectors, compares them and stores
//! the sign-extended mask, and must select the instructions the pinned `llc`
//! selects: the same mnemonics on the same lane arrangements.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `sext (icmp <pred> a, b)`, or against zero when `with_second` is false.
fn ir(
    pred: &str,
    vector: &str,
    with_second: bool,
) -> String {
    // `<4 x i32>`: the lane count and the lane width give the access's size.
    let mut parts = vector.trim_matches(['<', '>']).split(' ');
    let lanes: u32 = parts.next().unwrap().parse().unwrap();
    let width: u32 = parts.nth(1).unwrap().trim_start_matches('i').parse().unwrap();
    let align = (lanes * width / 8).min(16);
    let (load_y, rhs) = if with_second {
        (format!("  %y = load {vector}, ptr %b, align {align}\n"), "%y")
    } else {
        (String::new(), "zeroinitializer")
    };
    format!(
        "{HEAD}define void @f(ptr %d, ptr %a, ptr %b) {{\n\
         \x20 %x = load {vector}, ptr %a, align {align}\n{load_y}\
         \x20 %c = icmp {pred} {vector} %x, {rhs}\n\
         \x20 %s = sext <{lanes} x i1> %c to {vector}\n\
         \x20 store {vector} %s, ptr %d, align {align}\n\
         \x20 ret void\n}}\n"
    )
}

fn ours(source: &str) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, source).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    if !said.status.success() {
        return Err(String::from_utf8_lossy(&said.stderr).into_owned());
    }
    Ok(String::from_utf8(std::fs::read(&output).unwrap()).unwrap())
}

/// The pin's `llc`, where it is built.
fn pin(source: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let build = std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64"));
    let llc = PathBuf::from(build).join("bin/llc");
    if !llc.exists() {
        eprintln!("skipped the pin comparison: no {}", llc.display());
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("t.ll");
    std::fs::write(&input, source).unwrap();
    let said =
        Command::new(llc).args(["-mtriple=aarch64-unknown-linux-gnu", "-O2", "-o", "-"]).arg(&input).output().unwrap();
    assert!(said.status.success(), "{}", String::from_utf8_lossy(&said.stderr));
    Some(String::from_utf8(said.stdout).unwrap())
}

/// The instructions of a listing, each as its mnemonic, the arrangement of
/// its first vector operand and whether it takes a zero: `cmgt .4s`, `cmlt
/// .8b #0`.
fn shapes(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(name) = line.split_whitespace().next() else { continue };
        if name.starts_with('.') || name.starts_with("//") || name.ends_with(':') || name == "ret" {
            continue;
        }
        let arrangement = line.find(['.']).map_or("", |at| line[at..].split([',', ' ']).next().unwrap());
        let zero = if line.ends_with("#0") || line.ends_with("#0.0") { " #0" } else { "" };
        // Ours spells `movi.2d v0`, the pin `movi v0.2d`.
        let name = name.split('.').next().unwrap();
        *counts.entry(format!("{name} {arrangement}{zero}")).or_insert(0) += 1;
    }
    counts
}

/// Ours selects what the pin selects.
fn matches_the_pin(source: &str) -> String {
    let text = ours(source).unwrap_or_else(|why| panic!("refused: {why}\n{source}"));
    if let Some(pinned) = pin(source) {
        assert_eq!(shapes(&text), shapes(&pinned), "ours:\n{text}\npin:\n{pinned}");
    }
    text
}

const PREDICATES: [&str; 10] = ["eq", "ne", "sgt", "sge", "slt", "sle", "ugt", "uge", "ult", "ule"];

/// Every predicate on a 128-bit vector of each lane width is a `cmeq`,
/// `cmgt`, `cmge`, `cmhi` or `cmhs`, with swapped operands for `lt`/`le` and
/// an `mvn` for `ne`.
#[test]
fn integer_compares_of_128_bit_vectors_are_cm_instructions() {
    for vector in ["<16 x i8>", "<8 x i16>", "<4 x i32>", "<2 x i64>"] {
        for pred in PREDICATES {
            matches_the_pin(&ir(pred, vector, true));
        }
    }
}

/// A compare against zero is the `#0` form (`cmlt v0.4s, v0.4s, #0`).
#[test]
fn integer_compares_against_zero_are_the_zero_forms() {
    for vector in ["<16 x i8>", "<8 x i16>", "<4 x i32>", "<2 x i64>"] {
        for pred in PREDICATES {
            matches_the_pin(&ir(pred, vector, false));
        }
    }
}

/// The 64-bit vectors compare in `d` registers (`cmgt v0.8b`).
#[test]
fn integer_compares_of_64_bit_vectors_are_cm_instructions() {
    for vector in ["<8 x i8>", "<4 x i16>", "<2 x i32>"] {
        for pred in PREDICATES {
            matches_the_pin(&ir(pred, vector, true));
            matches_the_pin(&ir(pred, vector, false));
        }
    }
}

/// The pin's `opt -O2` and then `llc`: `ours` runs the middle end too, and
/// instcombine rewrites a select by `sge` into one by `slt` with the arms
/// swapped, which raw `llc` does not.
fn pin_optimized(source: &str) -> Option<String> {
    let home = std::env::var("HOME").unwrap_or_default();
    let build = std::env::var("LLVM_BUILD").unwrap_or_else(|_| format!("{home}/scratch/llvm-build-aarch64"));
    let opt = PathBuf::from(&build).join("bin/opt");
    if !opt.exists() {
        eprintln!("skipped the pin comparison: no {}", opt.display());
        return None;
    }
    let said = Command::new(opt)
        .args(["-O2", "-mtriple=aarch64-unknown-linux-gnu", "-S", "-o", "-", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(source.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    assert!(said.status.success());
    pin(&String::from_utf8(said.stdout).unwrap())
}

/// `select (icmp <pred> a, b), a, c`.
fn select_ir(
    pred: &str,
    vector: &str,
) -> String {
    let mut parts = vector.trim_matches(['<', '>']).split(' ');
    let lanes: u32 = parts.next().unwrap().parse().unwrap();
    let width: u32 = parts.nth(1).unwrap().trim_start_matches('i').parse().unwrap();
    let align = (lanes * width / 8).min(16);
    format!(
        "{HEAD}define void @f(ptr %a, ptr %b, ptr %c, ptr %d) {{\n\
         \x20 %x = load {vector}, ptr %a, align {align}\n\
         \x20 %y = load {vector}, ptr %b, align {align}\n\
         \x20 %m = icmp {pred} {vector} %x, %y\n\
         \x20 %t = load {vector}, ptr %c, align {align}\n\
         \x20 %s = select <{lanes} x i1> %m, {vector} %x, {vector} %t\n\
         \x20 store {vector} %s, ptr %d, align {align}\n\
         \x20 ret void\n}}\n"
    )
}

/// A vector `select` was refused ("vector with no register class"). Every
/// predicate on each 128-bit type is a compare and a `bit`/`bif`/`bsl`, as the
/// pin's `opt -O2 | llc` makes it.
#[test]
fn vector_selects_are_a_compare_and_a_bit_select() {
    for vector in ["<16 x i8>", "<8 x i16>", "<4 x i32>", "<2 x i64>"] {
        for pred in PREDICATES {
            let source = select_ir(pred, vector);
            let text = ours(&source).unwrap_or_else(|why| panic!("refused: {why}\n{source}"));
            if let Some(pinned) = pin_optimized(&source) {
                assert_eq!(shapes(&text), shapes(&pinned), "{pred} {vector}\nours:\n{text}\npin:\n{pinned}");
            }
        }
    }
}

/// `c[i] = a[i] < b[i] ? a[i] : b[i]` over `float` stayed a scalar loop of
/// `fcsel` (jpeg_single's clamp, +1.1M instructions at -O2): the loop is
/// vectorized into `fcm*` and a `bit`/`bif`/`bsl` over `.4s`.
#[test]
fn a_float_select_loop_is_vectorized() {
    let loop_ir = include_str!("../../../opt/llrm-transforms/tests/lv/float_select.ll");
    let text = ours(&format!("target triple = \"aarch64-unknown-linux-gnu\"\n{loop_ir}")).unwrap();
    let vector = |prefix: &str| text.lines().any(|line| line.trim().starts_with(prefix) && line.contains(".4s"));
    assert!(vector("fcm"), "no vector fcm:\n{text}");
    assert!(vector("bit") || vector("bif") || vector("bsl"), "no bit select:\n{text}");
}
