//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what the pinned `opt -passes=simplifycfg` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::simplified;
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified under a layout with native integers is `expected`, and
/// computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("n8:16:32:64").expect("a datalayout");
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(simplified(context, &spec, &mut declared, function), "{text}");
    declared.place(&mut module).expect("placed");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// `text` simplified makes no rotate, and computes what it did on `inputs`.
fn check_no_rotate(
    text: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("n8:16:32:64").expect("a datalayout");
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, &mut declared, function);
    declared.place(&mut module).expect("placed");
    assert!(!printed(&module).contains("fshl"), "{}", printed(&module));
    assert_eq!(results(&module, inputs), before);
}

/// The switch of @f on i32 with `cases` to @a (1) and @b (2), else @d (3).
fn sparse(cases: &[(i32, char)]) -> String {
    let arms: String = cases.iter().map(|(value, to)| format!("    i32 {value}, label %{to}\n")).collect();
    format!(
        "define i32 @f(i32 %x) {{\nb0:\n  switch i32 %x, label %d [\n{arms}  ]\n\na:\n  ret i32 1\n\nb:\n  ret i32 2\n\nd:\n  ret i32 3\n}}\n"
    )
}

const INPUTS: &[&[i128]] =
    &[&[5], &[6], &[9], &[17], &[21], &[133], &[134], &[0], &[-3], &[8], &[160], &[161], &[-8], &[104]];
const DECLARE: &str = "declare i32 @llvm.fshl.i32(i32, i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)\n";

/// The text of @f after the rotate: `head` up to the switch, then `cases`
/// (the reduced value and its target), the ret blocks merged by simplifycfg.
fn reduced(
    head: &str,
    cases: &[(i32, char)],
    a_edges: usize,
) -> String {
    let arms: String = cases
        .iter()
        .map(|(value, to)| format!("    i32 {value}, label %{}\n", if *to == 'a' { "common.ret" } else { "b" }))
        .collect();
    let a = vec!["[ 1, %b0 ]"; a_edges].join(", ");
    let on = if head.contains("%1 =") { "1" } else { "0" };
    format!(
        "define i32 @f(i32 %x) {{\nb0:\n{head}  switch i32 %{on}, label %d [\n{arms}  ]\n\ncommon.ret:\n  %common.op = phi i32 [ 2, %b ], [ 3, %d ], {a}\n  ret i32 %common.op\n\nb:\n  br label %common.ret\n\nd:\n  br label %common.ret\n}}\n\n{DECLARE}"
    )
}

/// Eight cases 4 apart, with outliers, stayed a sparse switch (a jump table
/// of 129 slots, 8 used): no rotate was made.
#[test]
fn test_cases_four_apart_are_a_rotate_and_a_dense_switch() {
    let text = sparse(&[(5, 'a'), (9, 'b'), (13, 'a'), (17, 'b'), (61, 'a'), (65, 'b'), (129, 'a'), (133, 'b')]);
    let head = "  %0 = sub i32 %x, 5\n  %1 = call i32 @llvm.fshl.i32(i32 %0, i32 %0, i32 30)\n";
    let cases = [(0, 'a'), (1, 'b'), (2, 'a'), (3, 'b'), (14, 'a'), (15, 'b'), (31, 'a'), (32, 'b')];
    check(&text, &reduced(head, &cases, 4), INPUTS);
}

/// Multiples of 8 with one far outlier stayed sparse; base 0 is dense at the
/// optsize threshold, so no subtract is made.
#[test]
fn test_multiples_of_eight_are_rotated_without_a_subtract() {
    let text = sparse(&[(8, 'a'), (16, 'b'), (24, 'a'), (32, 'b'), (40, 'a'), (48, 'b'), (56, 'a'), (160, 'b')]);
    let head = "  %0 = call i32 @llvm.fshl.i32(i32 %x, i32 %x, i32 29)\n";
    let cases = [(1, 'a'), (2, 'b'), (3, 'a'), (4, 'b'), (5, 'a'), (6, 'b'), (7, 'a'), (20, 'b')];
    check(&text, &reduced(head, &cases, 4), INPUTS);
}

/// Cases crossing zero, read as unsigned, look 4 billion apart: no rotate was
/// made.
#[test]
fn test_cases_crossing_zero_are_reduced_from_the_signed_minimum() {
    let text = sparse(&[(-8, 'a'), (-4, 'b'), (0, 'a'), (4, 'b'), (100, 'a'), (104, 'b')]);
    let head = "  %0 = sub i32 %x, -8\n  %1 = call i32 @llvm.fshl.i32(i32 %0, i32 %0, i32 30)\n";
    let cases = [(0, 'a'), (1, 'b'), (2, 'a'), (3, 'b'), (27, 'a'), (28, 'b')];
    check(&text, &reduced(head, &cases, 3), INPUTS);
}

/// A switch already dense (10% of its range used) was rotated anyway.
#[test]
fn test_a_dense_switch_is_left() {
    check_no_rotate(&sparse(&[(0, 'a'), (4, 'b'), (8, 'a'), (12, 'b'), (16, 'a'), (20, 'b')]), INPUTS);
}

/// A switch of three cases was rotated; SDAG makes no jump table of fewer
/// than four.
#[test]
fn test_a_switch_of_three_cases_is_left() {
    check_no_rotate(&sparse(&[(5, 'a'), (900, 'b'), (5000, 'a')]), INPUTS);
}
