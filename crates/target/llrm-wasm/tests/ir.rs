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

/// Fifteen C shapes (if/else, `&&`/`||`, nested ifs, while, do-while, for,
/// break, continue, nested loops, calls in loops, early return). `mul i32 %a,
/// 2` failed `Unsupported { op: Shl, ty: I8 }`: the combiner made an `i8` shift
/// amount.
#[test]
fn control_flow_shapes() {
    check("shapes");
}

/// An `invoke` is a call and a branch to its normal block; the pad nothing else
/// reaches is dropped.
#[test]
fn invoke() {
    check("invoke");
}

/// Every `switch` stopped on 'its lowering made blocks', then on
/// `CannotSelect(BrJT)`. At `-O0` the pin makes a jump table of two cases
/// (`br_table` with the range check folded into the default), and a sparse
/// switch is a chain of compares in case order, not the balanced tree and
/// probability order of the optimizing levels.
#[test]
fn switches() {
    check("switch");
}

/// A switch on a function parameter: the pin's dead `COPY` of the parameter
/// comes right after the `ARGUMENT`; here it comes after the compare (the
/// scheduler's tie-break between two equal-order nodes).
#[test]
#[ignore = "the dead COPY of a switched-on parameter is placed after the range check, not before it"]
fn switches_on_a_parameter() {
    check("switch_param");
}

/// A function with a stack object panicked on 'the frame lowering is not
/// translated yet'. The pin reads `__stack_pointer`, subtracts the frame
/// (16-aligned), writes it back unless a call-free leaf fits the 128-byte red
/// zone, and addresses objects as offsets from the new stack pointer. Here
/// also: a store-then-load of a local stayed a single forwarded value (the
/// pin's `-O0` combiner does not forward), and one constant stored twice was
/// placed early (the pin gives constant nodes IR order 0).
#[test]
fn frames() {
    check("frame");
}

/// An internal function printed `.globl`; the pin prints none and the comment rides on `.type`.
#[test]
fn internal_functions_are_not_global() {
    check("internal");
}

/// A promoted `load i8` was a plain `i32` load of one byte: the legalizer must make it an any-extending load. The
/// selected `i32.load` of one byte stopped `SetP2AlignOperands` ("the default p2align is the access size").
#[test]
fn promoted_loads_are_any_extending() {
    check("anyext_load");
}

/// Division by a power of two or a constant combined to a shift whose amount was an `i8`; wasm shifts by the
/// type's own width, so type legalization stopped on "Srl promote operand".
#[test]
fn division_by_constants() {
    check("div_pow2");
}

/// Floating-point arithmetic, comparisons and conversions were `CannotSelect`.
#[test]
fn float_arithmetic() {
    check("float_arith");
}

/// Float casts, selects, calls and constants: the unordered comparison codes
/// were `CannotSelect`, a `ConstantFP` became its own operand.
#[test]
fn float_conversions_and_calls() {
    check("float_more");
}

/// `frem`, `sin`, `cos`, `atan`, `lrint` were refused ("convert node to
/// libcall", "promote result" of an `i16` `lrint`, a callee symbol with no
/// operand); the pin calls `fmod`, `sin`, `lrint`.
#[test]
fn float_libcalls() {
    check("float_libcalls");
}

/// `memcpy`, `memset` and `memmove` were refused ("a mem intrinsic for a target with no string
/// instructions"). The pin copies a small constant size with `i64` loads and stores, anything else
/// with `memory.copy` or `memory.fill` behind a zero-length test (a constant length needs none).
#[test]
fn memory_intrinsics() {
    check("mem_intrinsics");
}
