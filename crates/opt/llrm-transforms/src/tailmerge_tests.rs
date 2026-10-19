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
