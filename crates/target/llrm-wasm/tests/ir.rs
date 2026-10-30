//! IR to assembly text through the whole backend, against the pin's `llc
//! -mtriple=wasm32 -O0 -fast-isel=false` (`~/scratch/llvm-build-wasm`,
//! `compare.sh` regenerates the goldens). Each `tests/ir/NAME.ll` has its
//! output in `NAME.s`.

use std::path::Path;

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

/// `compare.sh`'s `norm`: comments, trailing blanks, block labels and blank
/// lines are not compared.
fn norm(text: &str) -> String {
    text.lines()
        .map(|line| line.split('#').next().unwrap_or("").trim_end())
        .filter(|line| !line.is_empty() && !(line.starts_with(".LBB") && line.ends_with(':')))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn check(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ir");
    let source = std::fs::read_to_string(dir.join(format!("{name}.ll"))).expect("the .ll");
    let golden = std::fs::read_to_string(dir.join(format!("{name}.s"))).expect("the .s");
    let got = assembly(&source, Width::W32, Options::default()).unwrap_or_else(|why| panic!("{name}: {why}"));
    assert_eq!(norm(&got), norm(&golden), "{name}");
}

/// Every shift selected `CannotSelect(Shl)`: the legalizer wanted an `i8`
/// amount, wasm's is the type shifted.
#[test]
fn shifts() {
    check("shifts");
}

/// A call printed `call $callee` with no arguments: a glued unit lists its
/// bottom-most node first.
#[test]
fn calls() {
    check("call");
}

/// A load had no memory operand once selection morphed it in place, so p2align
/// could not be set.
#[test]
fn memory() {
    check("mem");
}

#[test]
fn compares() {
    check("cmp");
}

#[test]
fn arithmetic() {
    check("arith");
}

/// Global addresses fold into the load offset as the pin's `SelectAddrOperands`
/// does (LLVM 20's `llc` does not: it prints `i32.const arr` and `i32.load 8`,
/// so it is no oracle here).
#[test]
fn globals() {
    check("global");
}

/// A diamond, a skipped block and a counting loop: block order, `block`/`loop`
/// nesting, branch depths, `br_unless`.
#[test]
fn branches() {
    check("branch");
}

/// Two loop PHIs came out in source order: the pin copies the later PHI first,
/// so the locals were numbered the other way round and `local.get 4` was
/// missing from the result.
#[test]
fn phi_copy_order() {
    check("phis");
}

/// Fifteen C shapes (if/else, `&&`/`||`, nested ifs, while, do-while, for, break, continue, nested loops, calls in
/// loops, early return). `mul i32 %a, 2` failed `Unsupported { op: Shl, ty: I8 }`: the combiner made an `i8` shift amount.
#[test]
fn control_flow_shapes() {
    check("shapes");
}

/// An `invoke` is a call and a branch to its normal block; the pad nothing else reaches is dropped.
#[test]
fn invoke() {
    check("invoke");
}
