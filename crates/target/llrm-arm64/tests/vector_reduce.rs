//! `llvm.vector.reduce.*` was refused ("intrinsic call
//! llvm.vector.reduce.add.v4i32"): no `VECREDUCE_*` node, no split of a wide
//! operand, no `UADDV` lowering. Each case is a straight-line load, reduce,
//! return, and must select the instructions the pinned `llc` selects (same
//! mnemonics, same counts).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `i<ret>` reduced from `<n x i<lane>>` by `op`, loaded from the pointer
/// argument.
fn ir(
    op: &str,
    lanes: u32,
    lane: u32,
) -> String {
    let vector = format!("<{lanes} x i{lane}>");
    format!(
        "{HEAD}define i{lane} @f(ptr %p) {{\n\
         \x20 %x = load {vector}, ptr %p, align 16\n\
         \x20 %r = call i{lane} @llvm.vector.reduce.{op}.v{lanes}i{lane}({vector} %x)\n\
         \x20 ret i{lane} %r\n}}\n\
         declare i{lane} @llvm.vector.reduce.{op}.v{lanes}i{lane}({vector})\n"
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

/// The instructions of an assembly listing, by mnemonic.
fn mnemonics(text: &str) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(name) = line.split_whitespace().next() else { continue };
        if name.starts_with('.') || name.starts_with("//") || name.ends_with(':') {
            continue;
        }
        *counts.entry(name.to_owned()).or_insert(0) += 1;
    }
    counts
}

fn count(
    text: &str,
    mnemonic: &str,
    suffix: &str,
) -> usize {
    text.lines()
        .filter(|line| {
            line.trim().strip_prefix(mnemonic).is_some_and(|rest| rest.starts_with(' ') && rest.contains(suffix))
        })
        .count()
}

/// Ours selects what the pin selects: the same instructions, as many of each.
fn matches_the_pin(source: &str) -> String {
    let text = ours(source).unwrap_or_else(|why| panic!("refused: {why}\n{source}"));
    if let Some(pinned) = pin(source) {
        assert_eq!(mnemonics(&text), mnemonics(&pinned), "ours:\n{text}\npin:\n{pinned}");
    }
    text
}

/// `<4 x i32>` add: `addv s, v.4s`.
#[test]
fn add_v4i32_is_addv() {
    let text = matches_the_pin(&ir("add", 4, 32));
    assert_eq!(count(&text, "addv", ".4s"), 1, "{text}");
}

/// `<2 x i64>` add: `addp d, v.2d`.
#[test]
fn add_v2i64_is_addp() {
    let text = matches_the_pin(&ir("add", 2, 64));
    assert_eq!(count(&text, "addp", ".2d"), 1, "{text}");
}

/// The `i16` result is promoted to `i32`; the reduction is still `addv h`.
#[test]
fn add_v8i16_is_addv() {
    let text = matches_the_pin(&ir("add", 8, 16));
    assert_eq!(count(&text, "addv", ".8h"), 1, "{text}");
}

#[test]
fn add_v16i8_is_addv() {
    let text = matches_the_pin(&ir("add", 16, 8));
    assert_eq!(count(&text, "addv", ".16b"), 1, "{text}");
}

#[test]
fn smax_v4i32_is_smaxv() {
    let text = matches_the_pin(&ir("smax", 4, 32));
    assert_eq!(count(&text, "smaxv", ".4s"), 1, "{text}");
}

#[test]
fn umax_v4i32_is_umaxv() {
    let text = matches_the_pin(&ir("umax", 4, 32));
    assert_eq!(count(&text, "umaxv", ".4s"), 1, "{text}");
}

#[test]
fn smin_v8i16_is_sminv() {
    let text = matches_the_pin(&ir("smin", 8, 16));
    assert_eq!(count(&text, "sminv", ".8h"), 1, "{text}");
}

#[test]
fn umin_v16i8_is_uminv() {
    let text = matches_the_pin(&ir("umin", 16, 8));
    assert_eq!(count(&text, "uminv", ".16b"), 1, "{text}");
}

/// A 256-bit operand is split: the halves are added, then reduced.
#[test]
fn add_v8i32_adds_the_halves_then_addv() {
    let text = matches_the_pin(&ir("add", 8, 32));
    assert_eq!(count(&text, "add", ".4s"), 1, "{text}");
    assert_eq!(count(&text, "addv", ".4s"), 1, "{text}");
}

/// `v8i64` is four registers, `v16i64` eight: two and three rounds of the
/// split.
#[test]
fn add_v8i64_and_v16i64_add_the_halves_then_addp() {
    for (lanes, adds) in [(8, 3), (16, 7)] {
        let text = matches_the_pin(&ir("add", lanes, 64));
        assert_eq!(count(&text, "add", ".2d"), adds, "{text}");
        assert_eq!(count(&text, "addp", ".2d"), 1, "{text}");
    }
}

#[test]
fn add_v32i8_adds_the_halves_then_addv() {
    let text = matches_the_pin(&ir("add", 32, 8));
    assert_eq!(count(&text, "add", ".16b"), 1, "{text}");
    assert_eq!(count(&text, "addv", ".16b"), 1, "{text}");
}

/// `and`, `or` and `xor` combine the two `i64` halves, then the halves of that
/// (DEVIATE VEC8: the pin's first split is a 64-bit vector, which has no type
/// here): one combine per halving, as many as lanes are left to fold.
#[test]
fn bitwise_reductions_fold_the_halves_with_the_scalar_operation() {
    for (op, mnemonic) in [("and", "and"), ("or", "orr"), ("xor", "eor")] {
        for (lanes, lane, combines) in [(2, 64, 1), (4, 32, 2), (16, 8, 4)] {
            let text = ours(&ir(op, lanes, lane)).unwrap_or_else(|why| panic!("refused: {why}"));
            assert_eq!(count(&text, mnemonic, ""), combines, "{op} v{lanes}i{lane}:\n{text}");
        }
    }
}

/// The kind, lane width and lane count of each reduction run below.
const CASES: [(&str, u32, u32); 26] = [
    ("add", 8, 16),
    ("add", 16, 8),
    ("add", 32, 4),
    ("add", 64, 2),
    ("and", 8, 16),
    ("and", 16, 8),
    ("and", 32, 4),
    ("and", 64, 2),
    ("or", 8, 16),
    ("or", 16, 8),
    ("or", 32, 4),
    ("or", 64, 2),
    ("xor", 8, 16),
    ("xor", 16, 8),
    ("xor", 32, 4),
    ("xor", 64, 2),
    ("smax", 8, 16),
    ("smin", 16, 8),
    ("umax", 32, 4),
    ("umin", 8, 16),
    ("smin", 32, 4),
    ("umax", 16, 8),
    ("add", 32, 8),
    ("add", 8, 32),
    ("add", 64, 8),
    ("add", 64, 16),
];

fn run_command(command: &mut Command) -> String {
    let said = command.output().expect("the tool starts");
    assert!(said.status.success(), "{command:?}: {}", String::from_utf8_lossy(&said.stderr));
    String::from_utf8_lossy(&said.stdout).into_owned()
}

/// Each reduction, compiled here and run under qemu, returns what a scalar
/// loop over the same lanes returns, for lanes with every bit pattern's edge
/// (the sign, all ones, zero). Skipped without the cross gcc and qemu.
#[test]
fn the_reductions_return_what_a_scalar_loop_returns() {
    let tools =
        ["aarch64-linux-gnu-gcc", "qemu-aarch64"].iter().all(|t| Command::new(t).arg("--version").output().is_ok());
    if !tools {
        eprintln!("skipped: needs aarch64-linux-gnu-gcc and qemu-aarch64");
        return;
    }
    let mut module = String::from(HEAD);
    let mut driver = String::from("#include <stdint.h>\n#include <stdio.h>\n#include <string.h>\n");
    let mut checks = String::new();
    for &(op, bits, lanes) in &CASES {
        let name = format!("{op}_v{lanes}i{bits}");
        let vector = format!("<{lanes} x i{bits}>");
        module += &format!(
            "define i{bits} @{name}(ptr %p) {{\n  %x = load {vector}, ptr %p, align 16\n  %r = call i{bits} \
             @llvm.vector.reduce.{op}.v{lanes}i{bits}({vector} %x)\n  ret i{bits} %r\n}}\n\
             declare i{bits} @llvm.vector.reduce.{op}.v{lanes}i{bits}({vector})\n"
        );
        let unsigned = matches!(op, "umax" | "umin");
        let ty = format!("{}int{bits}_t", if unsigned { "u" } else { "" });
        let combine = match op {
            "add" => "(T)(acc + v[i])",
            "and" => "(T)(acc & v[i])",
            "or" => "(T)(acc | v[i])",
            "xor" => "(T)(acc ^ v[i])",
            "smax" | "umax" => "(v[i] > acc ? v[i] : acc)",
            _ => "(v[i] < acc ? v[i] : acc)",
        }
        .replace('T', &ty);
        driver += &format!("{ty} {name}(const void *);\n");
        checks += &format!(
            "  {{ const {ty} *v = (const {ty} *)buf; {ty} acc = v[0];\n    for (int i = 1; i < {lanes}; i++) acc = {combine};\n    \
             if ({name}(buf) != acc) {{ printf(\"{name} seed %d\\n\", seed); bad = 1; }} }}\n"
        );
    }
    driver += "int main(void) {\n  int bad = 0;\n  _Alignas(16) uint8_t buf[256];\n  for (int seed = 0; seed < 40; seed++) {\n    \
               uint32_t x = 12345u + seed * 2654435761u;\n    for (int i = 0; i < 256; i++) {\n      x = x * 1664525u + 1013904223u;\n      \
               buf[i] = seed == 37 ? 0xff : seed == 38 ? 0x80 : seed == 39 ? 0x7f : (uint8_t)(x >> 24);\n    }\n";
    driver += &checks;
    driver += "  }\n  return bad;\n}\n";
    let scratch = tempfile::tempdir().unwrap();
    let dir = scratch.path();
    let (ir, object, c, driver_object, program) =
        (dir.join("r.ll"), dir.join("r.o"), dir.join("d.c"), dir.join("d.o"), dir.join("r"));
    std::fs::write(&ir, module).unwrap();
    std::fs::write(&c, driver).unwrap();
    run_command(Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc")).arg("-o").arg(&object).arg(&ir));
    run_command(Command::new("aarch64-linux-gnu-gcc").args(["-O1", "-c", "-o"]).arg(&driver_object).arg(&c));
    run_command(
        Command::new("aarch64-linux-gnu-gcc").arg("-static").arg("-o").arg(&program).arg(&object).arg(&driver_object),
    );
    let ran = Command::new("qemu-aarch64").arg(&program).output().expect("qemu starts");
    assert!(ran.status.success(), "{}", String::from_utf8_lossy(&ran.stdout));
}

/// The kinds with no NEON lowering are refused by name, not miscompiled:
/// `mul` and the `v2i64` min and max wait for `expandVecReduce`.
#[test]
fn a_reduction_with_no_lowering_is_refused_by_name() {
    for (source, said) in [(ir("mul", 4, 32), "VecReduceMul"), (ir("smax", 2, 64), "VecReduceSMax")] {
        let why = ours(&source).expect_err("refused");
        assert!(why.contains(said), "{why}");
    }
}

/// `ADDXrr`/`ADDWrr` are pseudos, and their operands were not constrained to
/// the general register class: a reduced lane in a vector register reached the
/// scalar `add` as is (`ADDXrr: operands are not those of its form`), which
/// failed LoopVectorize's output for `huge` and `tile` at -O2.
#[test]
fn a_reduction_feeding_a_scalar_add_moves_the_lane_to_a_general_register() {
    for (bits, lanes) in [(64, 2), (64, 8), (32, 4)] {
        let vector = format!("<{lanes} x i{bits}>");
        let source = format!(
            "{HEAD}define i{bits} @f(ptr %p, i{bits} %a) {{\n\
             \x20 %x = load {vector}, ptr %p, align 16\n\
             \x20 %r = call i{bits} @llvm.vector.reduce.add.v{lanes}i{bits}({vector} %x)\n\
             \x20 %s = add i{bits} %r, %a\n\
             \x20 ret i{bits} %s\n}}\n\
             declare i{bits} @llvm.vector.reduce.add.v{lanes}i{bits}({vector})\n"
        );
        let text = matches_the_pin(&source);
        assert_eq!(count(&text, "fmov", ""), 1, "{text}");
    }
}
