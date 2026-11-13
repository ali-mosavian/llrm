//! Each body is MIR text; the answers are `opt -passes=simplifycfg`'s
//! (`tools/llvmport/simplifycfgcompare.py`).

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::simplified;
use crate::testing::{parsed, printed};
use crate::twoentryphi::Speculation;

fn simplify(text: &str) -> String {
    let mut module = parsed(text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, &mut declared, function);
    declared.place(&mut module).unwrap();
    printed(&module)
}

/// `p` branches to `t` or to `bb`, which is only a comparison branching to
/// `t` or `e`: the comparison runs in `p` and one branch goes to `t` or `e`
/// (`foldBranchToCommonDest`). Before, two branches tested `%a` and `%b`.
#[test]
fn test_a_comparison_block_with_a_common_successor_folds_into_its_predecessor() {
    assert_eq!(
        simplify(
            "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  br i1 %a, label %t, label %bb

bb:
  %b = icmp eq i16 %x, 0
  br i1 %b, label %t, label %e

t:
  call void @g1()
  ret void

e:
  call void @g2()
  ret void
}
"
        ),
        "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  %0 = icmp eq i16 %x, 0
  %or.cond = select i1 %a, i1 true, i1 %0
  br i1 %or.cond, label %t, label %e

common.ret:
  ret void

t:
  call void @g1()
  br label %common.ret

e:
  call void @g2()
  br label %common.ret
}
"
    );
}

/// The same shape with `noundef` parameters folds to an `or`, not a select:
/// `%0` cannot be poison, so `or i1 %a, %0` equals the two branches. The C
/// front end states `noundef` of its scalar parameters, so a C `a || b` was the
/// select form (a compare and a branch more on m16) while it stated none.
#[test]
fn test_noundef_parameters_make_the_or_form() {
    let out = simplify(
        "declare void @g1()

declare void @g2()

define void @f(i1 noundef %a, i16 noundef %x) {
p:
  br i1 %a, label %t, label %bb

bb:
  %b = icmp eq i16 %x, 0
  br i1 %b, label %t, label %e

t:
  call void @g1()
  ret void

e:
  call void @g2()
  ret void
}
",
    );
    assert!(out.contains("%or.cond = or i1 %a, %0") && !out.contains("select"), "{out}");
}

/// Two instructions besides the condition do not fit the budget of one.
#[test]
fn test_a_block_with_two_instructions_besides_the_condition_stays() {
    let text = "declare void @g1()

declare void @g2()

define void @f(i1 %a, i16 %x) {
p:
  br i1 %a, label %t, label %bb

bb:
  %m = mul i16 %x, 3
  %n = add i16 %m, 5
  %b = icmp eq i16 %n, 0
  br i1 %b, label %t, label %e

common.ret:
  ret void

t:
  call void @g1()
  br label %common.ret

e:
  call void @g2()
  br label %common.ret
}
";
    let text_in = text.replace("common.ret:\n  ret void\n\n", "").replace("br label %common.ret", "ret void");
    assert_eq!(simplify(&text_in), text);
}

/// A predecessor whose branch shares no successor with `bb`'s was asked
/// `safeToMergeTerminators` (a successor-set intersection) before the recipe
/// refused it: the recipe is asked first and no pair is checked.
#[test]
fn test_a_predecessor_sharing_no_successor_is_not_checked_for_merging() {
    let mut module = parsed(
        "define i16 @f(i16 %a, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %k = icmp eq i16 %a, 0
  br i1 %k, label %b3, label %b4

b2:
  ret i16 1

b3:
  ret i16 2

b4:
  ret i16 3
}
",
    );
    let outer = Outer::of(&module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let entry = function.entry().expect("an entry");
    let bb = function.successors(entry)[0];
    let before = crate::branchtocommon::SAFE_TO_MERGE_ASKS.with(|count| count.get());
    assert!(!crate::branchtocommon::folded(context, &spec, &mut declared, function, bb));
    assert_eq!(crate::branchtocommon::SAFE_TO_MERGE_ASKS.with(|count| count.get()), before);
}

fn two_length_tests(param: &str) -> String {
    format!(
        "define i32 @f(ptr noalias readonly {param}dereferenceable(8) %a, ptr noalias readonly {param}dereferenceable(8) %b, i16 %i) {{
b0:
  %x = load i16, ptr %a
  %c1 = icmp ult i16 %i, %x
  br i1 %c1, label %b1, label %e
b1:
  %y = load i16, ptr %b
  %c2 = icmp ult i16 %i, %y
  br i1 %c2, label %t, label %e
t:
  ret i32 1
e:
  ret i32 0
}}
"
    )
}

/// The second test's length load was speculated into the first block because
/// the speculation rule did not ask the pointer's alignment ("x86 reads
/// unaligned"): the zip loop `i < a.len && i < b.len` got a select, then
/// `hoistMinMax` made it one compare against a `umin`, where the pin keeps two
/// tests. `opt -passes=simplifycfg` keeps two branches for a parameter that is
/// `dereferenceable(8)` and not `align 2`, and one select for `align 2`.
#[test]
fn test_a_load_through_a_pointer_of_unknown_alignment_is_not_speculated() {
    let after = simplify(&two_length_tests(""));
    assert_eq!(after.matches("br i1").count(), 2, "{after}");
    assert!(!after.contains("select"), "{after}");
}

#[test]
fn test_a_load_through_a_pointer_aligned_as_the_load_is_speculated() {
    let after = simplify(&two_length_tests("align 2 "));
    assert_eq!(after.matches("br i1").count(), 0, "{after}");
    assert!(after.contains("select"), "{after}");
}

const WEIGHTED: &str = "declare void @g1()

declare void @g2()

define void @f(i1 noundef %a, i16 noundef %x) {
p:
  br i1 %a, label %t, label %bb, !prof !0

bb:
  %b = icmp eq i16 %x, 0
  br i1 %b, label %t, label %e

t:
  call void @g1()
  ret void

e:
  call void @g2()
  ret void
}
";

/// Inverting the predecessor's branch left its weights behind, and the folded
/// branch kept the predecessor's weights though its condition had changed:
/// the fold now computes the pin's `PredTrue*SuccTrue` pair.
#[test]
fn test_the_folded_branch_carries_the_merged_weights() {
    let out = simplify(&format!("{WEIGHTED}\n!0 = !{{!\"branch_weights\", i32 1, i32 9}}\n"));
    assert!(!out.contains("label %bb"), "{out}");
    // PBI `br %a, t, bb` (1, 9), BI unweighted (1, 1): (1 * 2 + 9 * 1, 9 * 1).
    assert!(out.contains("branch_weights\", i32 11, i32 9"), "{out}");
}

/// Weights on both branches merge to the pin's pair; a check that weighted only
/// the first branch could not tell that the second's were dropped.
#[test]
fn test_weights_on_both_branches_merge() {
    let both = WEIGHTED.replace("label %t, label %e\n", "label %t, label %e, !prof !1\n");
    let out = simplify(&format!(
        "{both}\n!0 = !{{!\"branch_weights\", i32 1, i32 9}}\n!1 = !{{!\"branch_weights\", i32 3, i32 7}}\n"
    ));
    assert!(!out.contains("label %bb"), "{out}");
    // PBI `br %a, t, bb` (1, 9), BI (3, 7): (1 * 10 + 9 * 3, 9 * 7).
    assert!(out.contains("branch_weights\", i32 37, i32 63"), "{out}");
}

/// A first branch decided 99.9% one way is not speculated through: the pin
/// asks `getPredictableBranchThreshold` before it folds.
#[test]
fn test_a_probably_decided_first_branch_is_not_folded_through() {
    let out = simplify(&format!("{WEIGHTED}\n!0 = !{{!\"branch_weights\", i32 999, i32 1}}\n"));
    assert!(out.contains("label %bb"), "{out}");
}

/// The predecessor is inverted to meet the successor; its weights went with
/// the arms in the pin (`InvertBranch` swaps successors), not stayed behind.
#[test]
fn test_inverting_the_predecessor_swaps_its_weights_before_they_merge() {
    let out = simplify(
        "declare void @g1()

declare void @g2()

define void @f(i1 noundef %a, i16 noundef %x) {
p:
  br i1 %a, label %e, label %bb, !prof !0

bb:
  %b = icmp eq i16 %x, 0
  br i1 %b, label %t, label %e

t:
  call void @g1()
  ret void

e:
  call void @g2()
  ret void
}

!0 = !{!\"branch_weights\", i32 1, i32 9}
",
    );
    // Inverted: (9, 1) for `br !a, bb, e`; BI unweighted (1, 1): (9 * 1, 1 * 2
    // + 9 * 1).
    assert!(out.contains("branch_weights\", i32 9, i32 11"), "{out}");
}
