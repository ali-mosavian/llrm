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

/// `SimplifyCondBranchToCondBranch` for a common destination with `phis` phis
/// and its conditional branches left after simplifying, with the interpreter's
/// results unchanged.
fn branches_left(phis: usize) -> usize {
    branches_left_on(phis, &llrm_mir::tti::Basic)
}

fn branches_left_on(
    phis: usize,
    tti: &dyn llrm_mir::tti::Tti,
) -> usize {
    let names = ["p", "q", "r", "s"];
    let mut body = String::new();
    for (at, name) in names.iter().take(phis).enumerate() {
        let (from_a, from_b) = (["%x", "%y", "%z", "%x"][at], ["%y", "%z", "%x", "%z"][at]);
        body += &format!("  %{name} = phi i16 [ {from_a}, %b0 ], [ {from_b}, %b1 ]\n");
    }
    let mut sum = "%p".to_owned();
    for (at, name) in names.iter().take(phis).enumerate().skip(1) {
        body += &format!("  %t{at} = add i16 {sum}, %{name}\n");
        sum = format!("%t{at}");
    }
    let text = format!(
        "define i16 @f(i1 %a, i1 %b, i16 %x, i16 %y, i16 %z) {{
b0:
  br i1 %a, label %d, label %b1

b1:
  br i1 %b, label %d, label %e

d:
{body}  ret i16 {sum}

e:
  ret i16 0
}}
"
    );
    let mut module = parsed(&text);
    let inputs: &[&[i128]] = &[&[0, 0, 1, 2, 3], &[1, 0, 1, 2, 3], &[0, 1, 4, 5, 6], &[1, 1, 7, 8, 9]];
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let options = Options { speculate_blocks: false, ..Default::default() };
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&options, context, &spec, &mut declared, function);
    assert_eq!(results(&module, inputs), before);
    printed(&module).matches("br i1").count()
}

/// `NumPhis > 2` is tested before the phi is counted: a destination with three
/// phis is still folded and only a fourth disables it. A limit of two left
/// `known_loads_walks` (wasm32 -O2) with branches the pin turns into selects,
/// 4337 B against 4109.
#[test]
fn test_a_destination_with_three_phis_is_still_folded_and_one_with_four_is_not() {
    assert_eq!(branches_left(3), 1, "three phis: one merged branch");
    assert_eq!(branches_left(2), 1, "two phis: one merged branch");
    assert_eq!(branches_left(4), 2, "four phis: both branches stay");
}

/// Where a `select` is a branch the limit stays two phis: three grew
/// `known_loads_walks` 2326 -> 2532 B on z80 -O2.
#[test]
fn test_without_a_conditional_move_a_destination_with_three_phis_keeps_its_branches() {
    let flagged = crate::testing::Flagged { no_cmov: true, ..Default::default() };
    assert_eq!(branches_left_on(3, &flagged), 2, "three phis, no cmov: both branches stay");
    assert_eq!(branches_left_on(2, &flagged), 1, "two phis, no cmov: one merged branch");
}
