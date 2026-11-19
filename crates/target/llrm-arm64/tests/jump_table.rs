//! A switch of ten cases to distinct blocks is a jump table: `MOVaddrJT`,
//! `JumpTableDest32` and a `BR` (`LowerJumpTable`, `LowerBR_JT`). The expected
//! opcodes are the pin's `llc -stop-after=finalize-isel`.

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

fn switch_ir() -> String {
    let mut ir = String::new();
    for i in 0..10 {
        ir += &format!("declare void @f{i}()\n");
    }
    ir += "define void @f(i32 %x) {\n  switch i32 %x, label %d [\n";
    for i in 0..10 {
        ir += &format!("    i32 {i}, label %l{i}\n");
    }
    ir += "  ]\n";
    for i in 0..10 {
        ir += &format!("l{i}:\n  call void @f{i}()\n  ret void\n");
    }
    ir + "d:\n  ret void\n}\n"
}

/// The legalizer refused the switch ("custom lowering of BrJT"); the pin's
/// block of the dispatch is the table address, the entry and the jump.
#[test]
fn a_ten_case_switch_dispatches_through_a_jump_table() {
    let blocks = selected(&switch_ir()).unwrap();
    assert!(blocks.contains(&vec!["MOVaddrJT", "JumpTableDest32", "JUMP_TABLE_DEBUG_INFO", "BR"]), "{blocks:?}");
}
