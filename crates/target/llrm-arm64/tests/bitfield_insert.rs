//! `tryBitfieldInsertOp`: an `or` of a masked destination and a positioned or
//! extracted field is one `BFM` (`BFI`, `BFXIL`). The expected opcodes are the
//! pin's `llc -stop-after=finalize-isel`.

use llrm_arm64::dag::function::{Selected, select_function};
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;

const DATALAYOUT: &str = "e-m:o-i64:64-i128:128-n32:64-S128";

fn names(one: &Selected) -> Vec<&'static str> {
    one.units
        .iter()
        .flat_map(|unit| unit.nodes.iter().rev().copied())
        .filter_map(|node| match one.dag.nodes()[node as usize].opcode {
            ISD::Machine(code) => MACHINE_OPCODES.iter().find(|(v, _)| u32::from(*v) == code).map(|(_, name)| *name),
            _ => None,
        })
        .collect()
}

fn selected(body: &str) -> Result<Vec<Vec<&'static str>>, String> {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{body}");
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    // The pipeline sees an atomic access as a call; the builder reads it back.
    llrm_mir::atomic::hidden(&mut module).expect("hides");
    Ok(select_function(&module, "f")?.iter().map(names).collect())
}

fn one(ir: &str) -> Vec<&'static str> {
    selected(ir).unwrap().concat()
}

/// The field `s << 4 & 0xf0` inserted into `d`: an `and`, a `ubfiz`, an `and`
/// and an `orr`; the pin has one `BFMWri`.
#[test]
fn a_shifted_field_inserted_into_a_masked_word_is_one_bfi() {
    let ir = "define i32 @f(i32 %d, i32 %s) {\n %m = and i32 %d, -241\n %t = shl i32 %s, 4\n %u = and i32 %t, 240\n %r = or i32 %m, %u\n ret i32 %r\n}";
    assert_eq!(one(ir), ["BFMWri", "RET_ReallyLR"]);
}

#[test]
fn a_low_field_inserted_into_a_masked_word_is_one_bfxil() {
    let ir = "define i32 @f(i32 %d, i32 %s) {\n %m = and i32 %d, -16\n %u = and i32 %s, 15\n %r = or i32 %m, %u\n ret i32 %r\n}";
    assert_eq!(one(ir), ["BFMWri", "RET_ReallyLR"]);
}

#[test]
fn a_field_shifted_right_into_place_is_a_ubfm_and_a_bfm() {
    let ir = "define i32 @f(i32 %d, i32 %s) {\n %m = and i32 %d, -4081\n %t = lshr i32 %s, 2\n %u = and i32 %t, 4080\n %r = or i32 %m, %u\n ret i32 %r\n}";
    assert_eq!(one(ir), ["UBFMWri", "BFMWri", "RET_ReallyLR"]);
}

#[test]
fn a_64_bit_field_is_a_bfmxri() {
    let ir = "define i64 @f(i64 %d, i64 %s) {\n %m = and i64 %d, -4278190081\n %t = shl i64 %s, 24\n %u = and i64 %t, 4278190080\n %r = or i64 %m, %u\n ret i64 %r\n}";
    assert_eq!(one(ir), ["BFMXri", "RET_ReallyLR"]);
}
