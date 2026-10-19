//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::simplified;
use crate::testing::{parsed, printed, results};
use crate::transform::BLOCKS_VISITED;
use crate::twoentryphi::Speculation;

/// @f simplified; whether anything changed.
fn simplify(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, function)
}

/// @f simplified under `options`; whether anything changed.
fn simplify_with(
    options: &super::Options,
    module: &mut Module,
) -> bool {
    let outer = Outer::of(module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    super::simplified_with(options, context, &spec, function)
}

/// `text` simplified is `expected`, and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    check_with(&super::Options::default(), text, expected, inputs);
}

/// [`check`] without `foldTwoEntryPHINode`: for the tests of the jump blocks,
/// whose diamonds it would turn into selects.
fn check_branching(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    check_with(&super::Options { speculate_blocks: false, ..Default::default() }, text, expected, inputs);
}

fn check_with(
    options: &super::Options,
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(simplify_with(options, &mut module), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A literal `br i1 true` left both arms to `decide` (2,298 of its 3,002
/// decisions on the C corpus), and its block and the join after it stayed
/// until a later `merged`.
#[test]
fn test_a_branch_on_true_is_a_jump_and_the_dead_arm_goes() {
    check(
        "define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
",
        "define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
",
        &[&[5], &[0]],
    );
}

#[test]
fn test_a_branch_on_false_takes_the_second_arm_and_the_join_loses_that_input() {
    check(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  br i1 false, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i16 %y) {
b0:
  ret i16 %y
}
",
        &[&[1, 2], &[3, 4]],
    );
}

/// Both arms to one block are two edges: the phi has two inputs from the
/// block, and one of them goes with the second edge.
#[test]
fn test_a_branch_to_one_block_twice_takes_one_phi_input_away_and_its_condition_goes() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %b1, label %b1

b1:
  %p = phi i16 [ %x, %b0 ], [ %x, %b0 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
",
        &[&[1], &[2]],
    );
}

#[test]
fn test_a_switch_on_a_constant_goes_to_its_case_or_its_default() {
    let body = |value: i16| {
        format!(
            "define i16 @f(i16 %x) {{
b0:
  switch i16 {value}, label %b3 [
    i16 1, label %b1
    i16 2, label %b2
  ]

b1:
  ret i16 %x

b2:
  ret i16 20

b3:
  ret i16 30
}}
"
        )
    };
    for (value, kept) in [(1, "ret i16 %x"), (2, "ret i16 20"), (7, "ret i16 30")] {
        check(&body(value), &format!("define i16 @f(i16 %x) {{\nb0:\n  {kept}\n}}\n"), &[&[9]]);
    }
}

/// A case to the default's block is no explicit compare; the one left is a
/// `br` on an equality.
#[test]
fn test_a_case_that_goes_where_the_default_goes_is_dropped_and_one_case_is_a_compare() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b2 [
    i16 1, label %b1
    i16 2, label %b2
  ]

b1:
  ret i16 10

b2:
  ret i16 20
}
",
        "define i16 @f(i16 %x) {
b0:
  %cond = icmp eq i16 %x, 1
  %0 = select i1 %cond, i16 10, i16 20
  ret i16 %0
}
",
        &[&[1], &[2], &[3]],
    );
}

/// Blocks only a folded branch reached, in a cycle of their own, go with
/// their uses.
#[test]
fn test_a_cycle_only_a_folded_branch_reached_is_deleted() {
    check(
        "define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  ret i16 %x

b2:
  %i = phi i16 [ 0, %b0 ], [ %n, %b3 ]
  br label %b3

b3:
  %n = add i16 %i, 1
  br label %b2
}
",
        "define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
",
        &[&[4]],
    );
}

/// The two returns are one (`tailMergeBlocksWithSimilarFunctionTerminators`),
/// and the two-entry phi of constants it makes a select.
#[test]
fn test_a_branch_on_a_value_whose_arms_return_constants_is_a_select() {
    check(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i16 10

b2:
  ret i16 20
}
",
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  %0 = select i1 %c, i16 10, i16 20
  ret i16 %0
}
",
        &[&[1], &[2]],
    );
}

/// The jump that closes a loop keeps its `!llvm.loop` (it is the loop's one
/// latch, so it stays).
#[test]
fn test_the_new_jump_keeps_the_loop_metadata() {
    let mut module = parsed(
        "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %n, %b2 ]
  %n = add i16 %i, 1
  %c = icmp ult i16 %n, %x
  br i1 %c, label %b2, label %b3

b2:
  br i1 true, label %b1, label %b3, !llvm.loop !0

b3:
  ret i16 %n
}

!0 = distinct !{!0, !{!\"llvm.loop.mustprogress\"}}
",
    );
    assert!(simplify(&mut module));
    assert!(printed(&module).contains("br label %b1, !llvm.loop"), "{}", printed(&module));
}

/// `decide` took out `phi [%x, entry], [%self, latch]` (gvn leaves them) when
/// it decided a branch; with the literal branches folded here first, `bintree`
/// -O3 kept the phis and its build loop came out 7 instructions longer.
#[test]
fn test_a_fold_takes_a_phi_of_itself_and_one_value_out_of_the_body() {
    let mut module = parsed(
        "define i16 @f(i16 %x, i16 %n) {
b0:
  br i1 true, label %b1, label %b4

b1:
  %p = phi i16 [ %x, %b0 ], [ %p, %b2 ]
  %i = phi i16 [ 0, %b0 ], [ %j, %b2 ]
  %j = add i16 %i, %p
  %c = icmp slt i16 %j, %n
  br i1 %c, label %b2, label %b3

b2:
  br label %b1

b3:
  ret i16 %j

b4:
  ret i16 0
}
",
    );
    let before = results(&module, &[&[1, 5], &[2, 9]]);
    assert!(simplify(&mut module));
    assert_eq!(printed(&module).matches("phi").count(), 1, "{}", printed(&module));
    assert_eq!(results(&module, &[&[1, 5], &[2, 9]]), before);
}

/// Blocks `_trivial_phis` looked at simplifying a chain of n folded branches,
/// each leaving a phi of one value.
fn chain_work(n: usize) -> usize {
    let mut text = String::from("define i16 @f(i16 %x) {\nb0:\n  br i1 true, label %b1, label %d0\n\n");
    for i in 0..n {
        let next =
            if i + 1 < n { format!("br i1 true, label %b{}, label %d{}", i + 2, i + 1) } else { "ret i16 %p".into() };
        text += &format!(
            "d{i}:\n  br label %b{}\n\nb{}:\n  %p{} = phi i16 [ %x, %b{} ], [ 0, %d{i} ]\n",
            i + 1,
            i + 1,
            i + 1,
            i
        );
        text += &format!("  {}\n\n", next.replace("%p", &format!("%p{}", i + 1)));
    }
    text += "}\n";
    let mut module = parsed(&text);
    BLOCKS_VISITED.with(|visited| visited.set(0));
    simplify(&mut module);
    BLOCKS_VISITED.with(std::cell::Cell::get)
}

/// The phi cleanup after the folds ran once per pass: 4x the blocks is 4x the
/// blocks looked at. Run after every fold it looked at 16x (the first version
/// of this test timed the pass and failed under host load; this counts).
/// Seen to fail first with `_trivial_phis` called after each fold.
#[test]
fn test_simplifying_a_chain_of_folds_looks_at_blocks_linearly_in_its_length() {
    let small = chain_work(100);
    let large = chain_work(400);
    assert!(small > 0 && large < small * 6, "100 blocks {small} blocks looked at, 400 blocks {large}: slope above 1.3");
}

/// A block holding only a jump left the CFG a hop longer than LLVM's
/// (`decide` bypassed it only where the target had no phi).
#[test]
fn test_a_block_that_only_jumps_is_bypassed_and_the_phi_after_it_follows() {
    check_branching(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  %y = add i16 %x, 1
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b3, label %b2

b2:
  %y = add i16 %x, 1
  br label %b3

b3:
  %p = phi i16 [ %y, %b2 ], [ %x, %b0 ]
  ret i16 %p
}
",
        &[&[3, 0], &[3, 1]],
    );
}

/// Both sides of a conflict: b0 reaches b2 itself with 1 and through b1 with
/// 2, so b1 must stay.
#[test]
fn test_a_jump_block_stays_where_a_shared_predecessor_would_send_two_values() {
    check_branching(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ 1, %b0 ], [ 2, %b1 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ 1, %b0 ], [ 2, %b1 ]
  ret i16 %p
}
",
        &[&[3, 0], &[3, 1]],
    );
}

/// A block with phis of its own and two predecessors is merged into its
/// successor's phis.
#[test]
fn test_a_block_of_phis_and_a_jump_is_merged_into_the_successor() {
    check_branching(
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br i1 %d, label %b3, label %b4

b2:
  br label %b3

b3:
  %q = phi i16 [ %x, %b1 ], [ 5, %b2 ]
  br label %b4

b4:
  %r = phi i16 [ %q, %b3 ], [ 7, %b1 ]
  ret i16 %r
}
",
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b0:
  br i1 %c, label %b1, label %b4

b1:
  br i1 %d, label %b3, label %b4

b3:
  br label %b4

b4:
  %r = phi i16 [ 7, %b1 ], [ 5, %b0 ], [ %x, %b3 ]
  ret i16 %r
}
",
        &[&[3, 0, 0], &[3, 0, 1], &[3, 1, 0], &[3, 1, 1]],
    );
}

/// The block that joins a loop's two ways back is its one latch; sent on to
/// the header from two blocks, the loop would lose its simple form
/// (`NeedCanonicalLoop`).
#[test]
fn test_the_one_latch_of_a_loop_is_not_bypassed_by_its_two_ways_back() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %m, %b4 ]
  %t = icmp ult i16 %i, %x
  br i1 %t, label %b2, label %b5

b2:
  %a = add i16 %i, 1
  %d = icmp ult i16 %a, 5
  br i1 %d, label %b3, label %b4

b3:
  %b = add i16 %i, 2
  br label %b4

b4:
  %m = phi i16 [ %a, %b2 ], [ %b, %b3 ]
  br label %b1

b5:
  ret i16 %i
}
";
    check_branching(text, text, &[&[0], &[7]]);
}

/// With `needCanonicalLoops` off (the slot after vectorization) the block that
/// joins a loop's two ways back is bypassed like any other; with it on
/// (`Options::default`, the early slot) it stays.
#[test]
fn test_the_option_that_clears_needcanonicalloops_lets_the_latch_go() {
    let text = "define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %m, %b4 ]
  %t = icmp ult i16 %i, %x
  br i1 %t, label %b2, label %b5

b2:
  %a = add i16 %i, 1
  %d = icmp ult i16 %a, 5
  br i1 %d, label %b3, label %b4

b3:
  %b = add i16 %i, 2
  br label %b4

b4:
  %m = phi i16 [ %a, %b2 ], [ %b, %b3 ]
  br label %b1

b5:
  ret i16 %i
}
";
    let mut module = parsed(text);
    let outer = Outer::of(&mut module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let (context, function) = module.function_mut("f").expect("@f");
    let options = super::Options { need_canonical_loop: false, ..Default::default() };
    assert!(super::simplified_with(&options, context, &spec, function));
    assert!(!printed(&module).contains("b4:"), "{}", printed(&module));
}

/// `m = a < b ? a : b` was a diamond of empty arms and a phi, and the
/// bisect of `d_draw_faces` found no `select` where clang has 18: no
/// SimplifyCFG here formed one.
#[test]
fn test_a_diamond_of_empty_arms_and_a_phi_is_a_select() {
    check(
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %c = icmp slt i16 %a, %b
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %m = phi i16 [ %a, %b1 ], [ %b, %b2 ]
  ret i16 %m
}
",
        "define i16 @f(i16 %a, i16 %b) {
b0:
  %c = icmp slt i16 %a, %b
  %0 = select i1 %c, i16 %a, i16 %b
  ret i16 %0
}
",
        &[&[1, 2], &[5, 3], &[4, 4]],
    );
}

/// The triangle's one cheap instruction runs before the branch; the phi
/// reads it through the select.
#[test]
fn test_a_triangle_with_a_cheap_instruction_hoists_it_and_selects() {
    check(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ %y, %b1 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  %y = add i16 %x, 1
  %0 = select i1 %c, i16 %y, i16 %x
  ret i16 %0
}
",
        &[&[3, 1], &[3, 0]],
    );
}

/// Five instructions cost more than the budget (4): the branch stays.
#[test]
fn test_an_arm_dearer_than_the_budget_keeps_its_branch() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %a = add i16 %x, 1
  %b = add i16 %a, 2
  %d = add i16 %b, 3
  %e = add i16 %d, 4
  %g = add i16 %e, 5
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ %g, %b1 ]
  ret i16 %p
}
";
    check(text, text, &[&[3, 1], &[3, 0]]);
}

/// A division by a variable may trap: it is not run where it did not run.
#[test]
fn test_an_arm_that_may_trap_keeps_its_branch() {
    let text = "define i16 @f(i16 %x, i16 %y, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %q = udiv i16 %x, %y
  br label %b2

b2:
  %p = phi i16 [ %x, %b0 ], [ %q, %b1 ]
  ret i16 %p
}
";
    check(text, text, &[&[6, 3, 1], &[6, 0, 0]]);
}

/// With `SpeculateBlocks` off no select forms: only the empty arm goes.
#[test]
fn test_without_speculate_blocks_no_select_forms() {
    // The arms differ, so `hoistCommonCodeFromSuccessors` has nothing to share.
    let text = "define i16 @f(i16 %a, i16 %b, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  %d = mul i16 %b, 3
  br label %b3

b3:
  %m = phi i16 [ %a, %b1 ], [ %d, %b2 ]
  ret i16 %m
}
";
    let expected = "define i16 @f(i16 %a, i16 %b, i1 %c) {
b0:
  br i1 %c, label %b3, label %b2

b2:
  %d = mul i16 %b, 3
  br label %b3

b3:
  %m = phi i16 [ %d, %b2 ], [ %a, %b0 ]
  ret i16 %m
}
";
    check_branching(text, expected, &[&[1, 2, 1], &[1, 2, 0]]);
}

/// The join has a third predecessor, so `foldTwoEntryPHINode` does not apply:
/// the add of the arm runs before the branch (`speculativelyExecuteBB`) and
/// the phi reads a select.
#[test]
fn test_one_cheap_instruction_of_a_one_armed_if_runs_before_the_branch() {
    check(
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b9:
  br i1 %d, label %b0, label %b2

b0:
  br i1 %c, label %b2, label %b1

b1:
  %y = add i16 %x, 1
  br label %b2

b2:
  %p = phi i16 [ 9, %b9 ], [ %x, %b0 ], [ %y, %b1 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b9:
  %y = add i16 %x, 1
  %0 = select i1 %c, i16 %x, i16 %y
  %1 = select i1 %d, i16 %0, i16 9
  ret i16 %1
}
",
        &[&[3, 1, 1], &[3, 0, 1], &[3, 1, 0]],
    );
}

/// A store is not run early (the pin only does that for a store it can prove
/// safe, which is not ported).
#[test]
fn test_a_store_in_the_one_armed_if_stays_behind_its_branch() {
    let text = "define i16 @f(i16 %x, i1 %c) {
b0:
  %q = alloca i16
  br i1 %c, label %b2, label %b1

b1:
  store i16 %x, ptr %q
  br label %b2

b2:
  ret i16 %x
}
";
    check(text, text, &[&[3, 1], &[3, 0]]);
}

/// `%t` is read by the arm alone: running the arm early leaves `%t` behind
/// as a second instruction speculated, so the branch stays.
#[test]
fn test_a_value_only_the_arm_reads_counts_as_a_second_speculated_instruction() {
    let text = "define i16 @f(i16 %x, i1 %c, i1 %d) {
b9:
  br i1 %d, label %b0, label %b2

b0:
  %t = add i16 %x, 1
  br i1 %c, label %b2, label %b1

b1:
  %y = add i16 %t, 3
  br label %b2

b2:
  %p = phi i16 [ 9, %b9 ], [ %x, %b0 ], [ %y, %b1 ]
  ret i16 %p
}
";
    check(text, text, &[&[3, 1, 1], &[3, 0, 1], &[3, 1, 0]]);
}

/// `CanMergeValues` and `replaceUndefValuesInPhi`: an `undef` input merges with
/// any other as `poison` does, so a jump block whose phi value is `undef` is
/// bypassed where b0 itself reaches b2 with 2. The second body runs the
/// `poison`/`undef` pair fix-up of `replaceUndefValuesInPhi`; the phi folds
/// afterwards either way, so no result tells the fix-up from its absence.
#[test]
fn test_undef_inputs_merge_and_a_poison_undef_pair_becomes_undef() {
    check_branching(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b2:
  %p = phi i16 [ undef, %b0 ], [ 2, %b1 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c) {
b0:
  ret i16 2
}
",
        &[&[3, 1]],
    );
    check_branching(
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b0:
  br i1 %d, label %b3, label %b4

b4:
  br i1 %c, label %b1, label %b2

b1:
  br label %b2

b3:
  br label %b2

b2:
  %p = phi i16 [ poison, %b4 ], [ undef, %b1 ], [ %x, %b3 ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x, i1 %c, i1 %d) {
b0:
  ret i16 %x
}
",
        &[&[3, 1, 1]],
    );
}
