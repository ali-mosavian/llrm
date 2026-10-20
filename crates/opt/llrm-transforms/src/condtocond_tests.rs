//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified is `expected`, and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let options = Options { speculate_blocks: false, ..Default::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified_with(&options, context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// `if (a || b) X else Y` written as two branches, the second in an empty
/// block: one branch on the `or`.
#[test]
fn test_an_empty_block_branching_to_its_predecessors_destination_is_one_branch() {
    check(
        "define i16 @f(i1 %a, i1 %b) {
b0:
  %m = alloca i16
  br i1 %a, label %d, label %b1

b1:
  br i1 %b, label %d, label %e

d:
  store i16 1, ptr %m
  ret i16 1

e:
  store i16 2, ptr %m
  ret i16 2
}
",
        "define i16 @f(i1 %a, i1 %b) {
b0:
  %m = alloca i16
  %or.cond = select i1 %a, i1 true, i1 %b
  br i1 %or.cond, label %d, label %e

common.ret:
  %common.op = phi i16 [ 1, %d ], [ 2, %e ]
  ret i16 %common.op

d:
  store i16 1, ptr %m
  br label %common.ret

e:
  store i16 2, ptr %m
  br label %common.ret
}
",
        &[&[0, 0], &[1, 0], &[0, 1], &[1, 1]],
    );
}
