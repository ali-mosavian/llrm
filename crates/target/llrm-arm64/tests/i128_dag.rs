//! `i128` on the DAG path alone: every operation the type legalizer splits into `i64` halves.
//!
//! Each of these was refused ("custom lowering of Load/Store/ShlParts/SrlParts/UAddOCarry/SetCCCarry", "expand a
//! division without a target") and the function fell back to the hand selector: 20 compiler-rt programs, `int128`,
//! `gen_div64`, `host_int128` and `gen_vectors64_edge` at the levels they build at.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

const LAYOUT: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

fn selects(body: &str) -> String {
    driver::set_isel_mode(IselMode::Dag);
    driver::assembly(&format!("{LAYOUT}{body}"), Platform::Linux, true).unwrap_or_else(|why| panic!("{why}"))
}

#[test]
fn an_i128_load_and_store_are_two_i64_accesses() {
    let asm = selects("define void @f(ptr %p, ptr %q) {\n  %v = load i128, ptr %p\n  store i128 %v, ptr %q\n  ret void\n}\n");
    assert!(asm.contains("ldp") && asm.contains("stp"), "{asm}");
}

#[test]
fn i128_shifts_are_expanded_into_i64_parts() {
    for op in ["shl", "lshr", "ashr"] {
        selects(&format!("define i128 @f(i128 %x, i128 %n) {{\n  %r = {op} i128 %x, %n\n  ret i128 %r\n}}\n"));
    }
}
