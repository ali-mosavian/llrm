//! A switch of ten cases whose default is a block with a phi: the table's gap
//! entries go straight to that block. The table's block was not a predecessor
//! of it, so those edges carried no phi value: QuickrBasic's `ctrl_stmt` read
//! an unwritten spill slot as its `false` and the compiler rejected `TIMER ON`.

use llrm_arm64::dag::function::select_function;
use llrm_codegen::backend::selectiondag::isd::ISD;
use llrm_dagisel::generated::aarch64::aarch64_apple_m1::MACHINE_OPCODES;

const DATALAYOUT: &str = "e-m:o-i64:64-i128:128-n32:64-S128";

fn switch_ir() -> String {
    let mut ir = String::new();
    ir += "define i32 @f(i32 %x) {\n  switch i32 %x, label %m [\n";
    for i in 0..10 {
        ir += &format!("    i32 {}, label %l{i}\n", i * 2);
    }
    ir += "  ]\n";
    for i in 0..10 {
        ir += &format!("l{i}:\n  br label %m\n");
    }
    ir += "m:\n  %r = phi i32 [ 0, %0 ]";
    for i in 0..10 {
        ir += &format!(", [ {}, %l{i} ]", i + 1);
    }
    ir + "\n  ret i32 %r\n}\n"
}

#[test]
fn the_jump_tables_block_is_a_predecessor_of_the_default_phi_block() {
    let text = format!("target datalayout = \"{DATALAYOUT}\"\n{}", switch_ir());
    let mut module = llrm_mir::parse::module(&text).expect("parses");
    llrm_mir::atomic::hidden(&mut module).expect("hides");
    let blocks = select_function(&module, "f").expect("selects");
    let merge = *blocks[0].layout.last().expect("a layout");
    let jumper = blocks
        .iter()
        .find(|one| {
            one.units
                .iter()
                .flat_map(|unit| unit.nodes.iter())
                .any(
                    |&node| matches!(
                        one.dag.nodes()[node as usize].opcode,
                        ISD::Machine(code) if MACHINE_OPCODES.iter().any(|(v, name)| u32::from(*v) == code && *name == "JumpTableDest32")
                    ),
                )
        })
        .expect("the block that jumps through the table");
    assert!(jumper.successors.contains(&merge), "{:?}", jumper.successors);
}
