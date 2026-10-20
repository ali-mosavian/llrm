//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified is `expected`, and computes what it did on `inputs`.
/// Without `foldTwoEntryPHINode`, which would make a select of the phi first.
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

/// `phi i1 [false, b0], [%c, b1]` then `br i1 %p`: b0 knows the answer and
/// went through b2 to run its compare again for nothing.
#[test]
fn test_a_predecessor_that_gives_the_phi_a_constant_skips_the_branch() {
    check(
        "define i16 @f(i1 %a, i16 %x) {
b0:
  %m = alloca i16
  br i1 %a, label %b1, label %b2

b1:
  %c = icmp sgt i16 %x, 0
  br label %b2

b2:
  %p = phi i1 [ false, %b0 ], [ %c, %b1 ]
  br i1 %p, label %b3, label %b4

b3:
  store i16 1, ptr %m
  ret i16 1

b4:
  store i16 2, ptr %m
  ret i16 2
}
",
        "define i16 @f(i1 %a, i16 %x) {
b0:
  %m = alloca i16
  br i1 %a, label %b1, label %b4

b1:
  %c = icmp sgt i16 %x, 0
  br i1 %c, label %b3, label %b4

common.ret:
  %common.op = phi i16 [ 1, %b3 ], [ 2, %b4 ]
  ret i16 %common.op

b3:
  store i16 1, ptr %m
  br label %common.ret

b4:
  store i16 2, ptr %m
  br label %common.ret
}
",
        &[&[0, 5], &[1, 5], &[1, -5], &[0, -5]],
    );
}

/// A predecessor that branches on the condition itself knows it on its edges.
#[test]
fn test_a_predecessor_that_branches_on_the_condition_threads_its_edge() {
    check(
        "define i16 @f(i16 %x, i1 %a) {
b0:
  %m = alloca i16
  %c = icmp sgt i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  br i1 %a, label %b2, label %b3

b2:
  br i1 %c, label %b4, label %b5

b3:
  store i16 3, ptr %m
  ret i16 3

b4:
  store i16 4, ptr %m
  ret i16 4

b5:
  store i16 5, ptr %m
  ret i16 5
}
",
        "define i16 @f(i16 %x, i1 %a) {
b0:
  %m = alloca i16
  %c = icmp sgt i16 %x, 0
  %not = xor i1 %c, true
  %or.cond = select i1 %not, i1 true, i1 %a
  br i1 %or.cond, label %b2, label %b3

b2:
  br i1 %c, label %b4, label %b5

common.ret:
  %common.op = phi i16 [ 3, %b3 ], [ 4, %b4 ], [ 5, %b5 ]
  ret i16 %common.op

b3:
  store i16 3, ptr %m
  br label %common.ret

b4:
  store i16 4, ptr %m
  br label %common.ret

b5:
  store i16 5, ptr %m
  br label %common.ret
}
",
        &[&[1, 0], &[1, 1], &[-1, 0], &[-1, 1]],
    );
}

/// A block of twelve instructions is not copied for each predecessor.
#[test]
fn test_a_block_too_big_to_copy_is_not_threaded() {
    let stores = "  store i16 %x, ptr %m\n".repeat(11);
    let text = format!(
        "define i16 @f(i1 %a, i16 %x) {{
b0:
  %m = alloca i16
  br i1 %a, label %b1, label %b2

b1:
  %c = icmp sgt i16 %x, 0
  br label %b2

b2:
  %p = phi i1 [ false, %b0 ], [ %c, %b1 ]
{stores}  br i1 %p, label %b3, label %b4

b3:
  store i16 1, ptr %m
  ret i16 1

b4:
  store i16 2, ptr %m
  ret i16 2
}}
"
    );
    let mut module = parsed(&text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let options = Options { speculate_blocks: false, ..Default::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&options, context, &spec, &mut declared, function);
    let out = printed(&module);
    assert!(out.contains("br i1 %a, label %b1, label %b2"), "{out}");
}
