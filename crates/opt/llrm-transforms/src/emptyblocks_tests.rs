//! A block of phis and a jump is merged into its successor.

use crate::cgpaddr::CgpAddr;
use crate::testing::{parsed, printed};

const HEAD: &str = "target datalayout = \"e-p:16:16-i32:16-n8:16:32\"\n\n";

fn after(text: &str) -> String {
    let mut module = parsed(&format!("{HEAD}{text}"));
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(CgpAddr);
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    printed(&module)
}

/// The block between a branch's arm and a join holds only a jump: the arm
/// jumps to the join (queens at m32 -O2: 6 such blocks, 0 after the pin's
/// CodeGenPrepare).
#[test]
fn test_an_empty_block_before_a_join_is_merged_into_the_join() {
    let text = after(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b3

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  br label %b3

b3:
  %p = phi i16 [ %y, %b2 ], [ %x, %b0 ]
  ret i16 %p
}
",
    );
    assert!(!text.contains("b2:"), "the empty block is gone: {text}");
    assert!(text.contains("[ %y, %b1 ]"), "the join's phi names the arm: {text}");
}

/// A block both it and another predecessor reach with different values is not
/// merged: the join's phi would have two values from one block.
#[test]
fn test_a_block_whose_merge_would_give_a_phi_two_values_stays() {
    let source = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ 1, %b0 ], [ 2, %b1 ]
  ret i16 %p
}
";
    assert_eq!(after(source), printed(&parsed(&format!("{HEAD}{source}"))));
}
