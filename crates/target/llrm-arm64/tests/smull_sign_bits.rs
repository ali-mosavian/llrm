//! `smullwithsignbits`: a 64-bit multiply of two values with more than 32 sign
//! bits is one `SMADDL` (`smull`). The expected opcodes are the pin's `llc
//! -stop-after=finalize-isel`.

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

/// `ComputeNumSignBits` was a query the DAG lacked, so the predicate answered
/// false and the multiply stayed a `MADDXrrr` (the pin has `SMADDLrrr`).
#[test]
fn a_multiply_of_two_values_with_more_than_32_sign_bits_is_a_smull() {
    let ir = "define i64 @f(i64 %x, i64 %y) {\n %a = ashr i64 %x, 32\n %b = ashr i64 %y, 32\n %r = mul i64 %a, %b\n ret i64 %r\n}";
    let ops = one(ir);
    assert!(ops.contains(&"SMADDLrrr"), "{ops:?}");
    assert!(!ops.contains(&"MADDXrrr"), "{ops:?}");
}

/// A value another block extended reaches the multiply as a register with an
/// `AssertSext`; the assertion was dropped before matching, so the predicate
/// saw no sign bits and `switch`'s `digit * digit` stayed a `MADDXrrr` (the
/// pin has `SMADDLrrr`).
#[test]
fn a_multiply_of_a_value_another_block_sign_extended_is_a_smull() {
    let ir = "define i64 @f(i32 %n) {\n\
        br label %h\n\
        h:\n %i = phi i32 [ 0, %0 ], [ %in, %j ]\n %acc = phi i64 [ 0, %0 ], [ %r, %j ]\n\
        %q = srem i32 %i, 10\n %s = sext i32 %q to i64\n %c = icmp ult i32 %q, 5\n br i1 %c, label %t, label %j\n\
        t:\n br label %j\n\
        j:\n %m = mul nsw i64 %s, %s\n %r = add i64 %m, %acc\n %in = add i32 %i, 1\n\
        %e = icmp eq i32 %in, %n\n br i1 %e, label %x, label %h\n\
        x:\n ret i64 %r\n}";
    let blocks = selected(ir).unwrap();
    let ops: Vec<&str> = blocks.concat();
    assert!(ops.contains(&"SMADDLrrr"), "{ops:?}");
    assert!(!ops.contains(&"MADDXrrr"), "{ops:?}");
}
