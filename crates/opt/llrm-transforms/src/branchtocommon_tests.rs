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
    let (context, function) = module.function_mut("f").expect("@f");
    let entry = function.entry().expect("an entry");
    let bb = function.successors(entry)[0];
    let before = crate::branchtocommon::SAFE_TO_MERGE_ASKS.with(|count| count.get());
    assert!(!crate::branchtocommon::folded(context, &spec, function, bb));
    assert_eq!(crate::branchtocommon::SAFE_TO_MERGE_ASKS.with(|count| count.get()), before);
}
