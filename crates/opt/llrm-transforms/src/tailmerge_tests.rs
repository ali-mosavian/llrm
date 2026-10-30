//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::module::Module;

use crate::testing::{parsed, printed, results};

fn merge(module: &mut Module) -> bool {
    let (context, function) = module.function_mut("f").expect("@f");
    super::merged(context, function)
}

/// Two blocks that return different values are one block that returns a phi
/// of them: LLVM's `common.ret`.
#[test]
fn test_two_returns_become_one_block_returning_a_phi() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  ret i16 %a

b2:
  %b = add i16 %x, 2
  ret i16 %b
}
",
    );
    let inputs: &[&[i128]] = &[&[3, 1], &[3, 0]];
    let before = results(&module, inputs);
    assert!(merge(&mut module));
    assert_eq!(
        printed(&module),
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

common.ret:
  %common.op = phi i16 [ %a, %b1 ], [ %b, %b2 ]
  ret i16 %common.op

b1:
  %a = add i16 %x, 1
  br label %common.ret

b2:
  %b = add i16 %x, 2
  br label %common.ret
}
"
    );
    assert_eq!(results(&module, inputs), before);
}

/// One block that returns has nothing to merge with.
#[test]
fn test_a_single_return_stays() {
    let text = "define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
";
    let mut module = parsed(text);
    assert!(!merge(&mut module));
    assert_eq!(printed(&module), text);
}

/// The operand slots rewritten while `blocks` returning blocks are merged.
fn rewritten_slots(blocks: usize) -> usize {
    let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  switch i16 %x, label %r0 [\n");
    for i in 1..blocks {
        text += &format!("    i16 {i}, label %r{i}\n");
    }
    text += "  ]\n\n";
    for i in 0..blocks {
        text += &format!("r{i}:\n  %a{i} = add i16 %x, {}\n  ret i16 %a{i}\n\n", i + 1);
    }
    text += "}\n";
    let mut module = parsed(&text);
    let (context, function) = module.function_mut("f").expect("@f");
    let _ = function.take_changes();
    assert!(super::merged(context, function));
    function
        .take_changes()
        .iter()
        .map(|change| match change {
            llrm_mir::module::Change::Operands { was, .. } => was.len(),
            _ => 0,
        })
        .sum()
}

/// Merging the returns rewrites each operand slot of the new phi once: 4x the
/// returns is 4x the slots. Rewriting the phi's operand list for each block
/// copied it every time (n^2 slots: 20,000 at 200 blocks).
#[test]
fn test_merging_returns_rewrites_operand_slots_linearly_in_their_number() {
    let (small, large) = (rewritten_slots(100), rewritten_slots(400));
    assert!(large <= 4 * small.max(400), "100 returns rewrote {small} slots, 400 rewrote {large}");
    assert!(large < 8 * 400, "400 returns rewrote {large} operand slots");
}
