//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::module::Module;
use llrm_mir::passes::{Declared, Outer};

use super::simplified;
use crate::testing::{parsed, printed, results};
use crate::transform::BLOCKS_VISITED;
use crate::twoentryphi::Speculation;

/// @f simplified; whether anything changed.
fn simplify(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, &spec, &mut declared, function)
}

/// @f simplified under `options`; whether anything changed.
fn simplify_with(
    options: &super::Options,
    module: &mut Module,
) -> bool {
    let outer = Outer::of(module, None);
    let layout = DataLayout::default();
    let spec = Speculation { tti: &llrm_mir::tti::Basic, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    super::simplified_with(options, context, &spec, &mut declared, function)
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

/// `int f(void) { for (;;) ; }` is an entry that jumps to a block that jumps
/// to itself. simplifycfg never finished with it: the compile hung at -O1 and
/// above.
#[test]
fn test_an_empty_infinite_loop_is_left_as_it_is() {
    let text = "define void @f() {
b0:
  br label %b1

b1:
  br label %b1
}
";
    for options in [super::Options::L1438, super::Options::L1682] {
        check_with(&options, text, text, &[]);
    }
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
/// successor's phis; the branches then fold into one
/// (`SimplifyCondBranchToCondBranch`, not gated by `SpeculateBlocks`), as `opt
/// -passes='simplifycfg<no-speculate-blocks>'` does.
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
  %not = xor i1 %c, true
  %not1 = xor i1 %d, true
  %or.cond = select i1 %not, i1 true, i1 %not1
  %mux = select i1 %not, i16 5, i16 7
  br i1 %or.cond, label %b4, label %b3

b3:
  br label %b4

b4:
  %r = phi i16 [ %x, %b3 ], [ %mux, %b0 ]
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
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let options = super::Options { need_canonical_loop: false, ..Default::default() };
    assert!(super::simplified_with(&options, context, &spec, &mut declared, function));
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

/// The store of the arm runs before the branch, of the value it stores or
/// the one the location already held (`HoistCondStores`): the location is
/// known to be written, by the store just before the branch.
#[test]
fn test_a_store_over_a_store_just_before_the_branch_runs_early() {
    check(
        "define i16 @f(i16 %x, i1 %c) {
entry:
  %a = alloca i16
  store i16 0, ptr %a
  br i1 %c, label %then, label %end

then:
  store i16 %x, ptr %a
  br label %end

end:
  %r = load i16, ptr %a
  ret i16 %r
}
",
        "define i16 @f(i16 %x, i1 %c) {
entry:
  %a = alloca i16
  store i16 0, ptr %a
  %spec.store.select = select i1 %c, i16 %x, i16 0
  store i16 %spec.store.select, ptr %a
  %r = load i16, ptr %a
  ret i16 %r
}
",
        &[&[7, 1], &[7, 0]],
    );
}

/// A load of an alloca is safe to run early: the join has a third
/// predecessor, so the two-entry fold does not apply, and the phi is a select.
#[test]
fn test_a_load_of_an_alloca_runs_before_the_branch() {
    check(
        "define i16 @f(i1 %c, i1 %d) {
b9:
  %a = alloca i16
  store i16 5, ptr %a
  br i1 %d, label %b0, label %end

b0:
  br i1 %c, label %then, label %end

then:
  %l = load i16, ptr %a
  br label %end

end:
  %r = phi i16 [ 9, %b9 ], [ 0, %b0 ], [ %l, %then ]
  ret i16 %r
}
",
        "define i16 @f(i1 %c, i1 %d) {
b9:
  %a = alloca i16
  store i16 5, ptr %a
  %l = load i16, ptr %a
  %0 = select i1 %c, i16 %l, i16 0
  %1 = select i1 %d, i16 %0, i16 9
  ret i16 %1
}
",
        &[&[1, 1], &[0, 1], &[1, 0]],
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

/// The late slot L1438 has `needCanonicalLoops(false)`. An empty block of two
/// predecessors that jumps to a loop header stays at the default (it would
/// give the header a third entry) and goes at the slot: `opt -passes=
/// 'simplifycfg<no-keep-loops;hoist-common-insts;sink-common-insts>'` makes
/// the same (tools/llvmport/simplifycfgcompare.py --no-keep-loops).
#[test]
fn test_an_empty_entry_block_of_a_loop_goes_only_without_canonical_loops() {
    let text = "@g = internal global i16 0

define i16 @f(i16 %n, i1 %c) {
entry:
  br i1 %c, label %e, label %a

a:
  %t = add i16 %n, 1
  store i16 %t, ptr @g
  br label %e

e:
  br label %h

h:
  %i = phi i16 [ 0, %e ], [ %i1, %h ]
  %i1 = add i16 %i, 1
  %k = icmp slt i16 %i1, %n
  br i1 %k, label %h, label %x

x:
  ret i16 %i1
}
";
    let inputs: &[&[i128]] = &[&[3, 1], &[3, 0], &[1, 1]];
    let run = |options: &super::Options| {
        let mut module = parsed(text);
        let before = results(&module, inputs);
        let changed = simplify_with(options, &mut module);
        assert_eq!(results(&module, inputs), before);
        (changed, printed(&module))
    };
    let (kept_changed, kept) = run(&super::Options::default());
    assert!(!kept_changed && kept.contains("\ne:\n"), "{kept}");
    // The options the pipeline builds the slot from.
    let (gone_changed, gone) = run(&super::Options::L1438);
    assert!(gone_changed && !gone.contains("\ne:\n"), "{gone}");
    assert!(gone.contains("br i1 %c, label %h, label %a"), "{gone}");
}

/// The loop-form blocks were derived by `loops_of` at each run though the
/// manager held the function's loops (d_faces -O2: 36 asks, 4.9 M Ir). With the
/// CFG as the manager found it they are read from its loops: none is derived.
#[test]
fn test_the_loop_form_blocks_come_from_the_managers_loops_while_the_cfg_stands() {
    let mut module = crate::testing::parsed(
        "define i16 @f(i16 %n, i1 %c, i1 %d) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b4 ]
  br i1 %c, label %b2, label %b3

b2:
  br label %b4

b3:
  br i1 %d, label %b4, label %b5

b4:
  %j = add i16 %i, 1
  %k = icmp slt i16 %j, %n
  br i1 %k, label %b1, label %b5

b5:
  ret i16 %j
}
",
    );
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(crate::simplifycfg::SimplifyCfg::default());
    let before = llrm_analysis::cfg::shapes_derived();
    manager.run_module(&mut module, std::rc::Rc::new(llrm_mir::target::Neutral)).expect("runs");
    let derived = llrm_analysis::cfg::shapes_derived() - before;
    assert_eq!(derived, 0, "the pass derived loops the manager held");
}

/// The head of every block visit asks only whether a block is reached by
/// nothing, by itself alone, or by another; it built the list of predecessors
/// for that (9,955 lists in d_faces -O2). The three answers.
#[test]
fn test_a_block_is_reached_by_nothing_by_itself_or_by_another() {
    let mut module = crate::testing::parsed(
        "define void @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b1

b2:
  ret void

b3:
  ret void
}
",
    );
    let (_, function) = module.function_mut("f").expect("@f");
    let blocks = function.layout().to_vec();
    use super::Reach;
    assert!(super::reached_from(function, blocks[0]) == Reach::Nothing, "the entry");
    assert!(super::reached_from(function, blocks[3]) == Reach::Nothing, "a block nothing names");
    assert!(super::reached_from(function, blocks[2]) == Reach::Other, "reached by b0");
    // b1 is named by b0 and by itself.
    assert!(super::reached_from(function, blocks[1]) == Reach::Other, "reached by b0 and itself");
}

/// Every block ran every conditional-branch step: a jump or a return paid the
/// shape checks of the compare chain, branch-to-common, known-predecessor and
/// the rest (3,661 visits of each in d_faces -O2, 1,925 of them on a
/// conditional branch). Only a conditional branch reaches them now.
#[test]
fn test_only_a_conditional_branch_runs_the_conditional_steps() {
    let mut module = crate::testing::parsed(
        "define i32 @f(i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  ret i32 1

b3:
  ret i32 2
}
",
    );
    super::CONDITIONAL_VISITS.with(|one| one.set(0));
    simplify_with(&super::Options { speculate_blocks: false, ..Default::default() }, &mut module);
    let visits = super::CONDITIONAL_VISITS.with(std::cell::Cell::get);
    assert!(visits >= 1, "the entry's branch is a conditional branch");
    assert!(visits <= 2, "b1, b2 and b3 end in a jump or a return: {visits} visits");
}

/// x_life -O1 kept two identical `icmp sgt n, 2`, one per loop guard LICM
/// hoisted, in the logical-or form of `foldBranchToCommonDest`; the pin's
/// `simplifyCondBranch` makes the second branch a constant first, from the
/// condition of its dominating predecessor, and keeps one compare.
#[test]
fn test_a_branch_the_dominating_conditions_imply_is_a_jump_and_its_compare_goes() {
    let mut module = parsed(
        "declare void @g()\ndefine void @f(i32 %n) {\nentry:\n  %a = icmp sgt i32 %n, 2\n  %b = icmp sgt i32 %n, 2\n  br i1 %a, label %mid, label %out\nmid:\n  br i1 %b, label %in, label %out\nin:\n  call void @g()\n  br label %out\nout:\n  ret void\n}\n",
    );
    assert!(simplify(&mut module));
    let out = printed(&module);
    assert_eq!(out.matches("icmp").count(), 1, "{out}");
    assert!(out.contains("br i1 %a, label %in, label %out"), "{out}");
}

/// A branch no dominating condition implies keeps its compare.
#[test]
fn test_a_branch_the_dominating_conditions_do_not_imply_keeps_its_compare() {
    let mut module = parsed(
        "declare void @g()\ndefine void @f(i32 %n) {\nentry:\n  %a = icmp sgt i32 %n, 2\n  %b = icmp sgt i32 %n, 5\n  br i1 %a, label %mid, label %out\nmid:\n  br i1 %b, label %in, label %out\nin:\n  call void @g()\n  br label %out\nout:\n  ret void\n}\n",
    );
    simplify(&mut module);
    let out = printed(&module);
    assert!(out.contains("icmp sgt i32 %n, 5"), "{out}");
}

/// A ladder of `n` conditional blocks, each reached from the one before and
/// leaving to a common exit.
fn ladder(n: usize) -> String {
    let mut text = String::from("declare i1 @h(i16)\ndefine i16 @f(i16 %x) {\nb0:\n  br label %c0\n");
    for i in 0..n {
        text += &format!("c{i}:\n  %v{i} = call i1 @h(i16 {i})\n  br i1 %v{i}, label %c{}, label %e\n", i + 1);
    }
    text += &format!("c{n}:\n  ret i16 1\ne:\n  ret i16 0\n}}\n");
    text
}

/// A block with one predecessor asked for its predecessor's predecessor by
/// building a list (`all_predecessors_come_from_same_source`): 4 lists a block
/// over the pass's two sweeps for 2 (5,723 lists, 1.05 M Ir in d_faces -O2,
/// beside the pin's 0.05 M). Seen to fail first with the list built (398 for
/// 100 blocks).
#[test]
fn test_a_ladder_of_conditional_blocks_builds_two_predecessor_lists_a_block() {
    let mut module = parsed(&ladder(100));
    super::PREDECESSOR_LISTS.with(|lists| lists.set(0));
    simplify(&mut module);
    let lists = super::PREDECESSOR_LISTS.with(std::cell::Cell::get);
    assert!((100..=250).contains(&lists), "100 blocks, {lists} predecessor lists: 4 a block would be 400");
}

/// The blocks that hold a loop's form up were found for every function with a
/// loop (each loop's exits scanned: 197 scans, 1.5 M Ir in d_faces -O2, the
/// pin has none); they are found when a jump alone in a block comes up.
/// Seen to fail first with the set found at the start of every pass.
#[test]
fn test_a_loop_with_no_jump_alone_in_a_block_scans_no_exits() {
    use llrm_mir::counts::{Event, get};
    let mut module = parsed(
        "declare i1 @h(i16)
define i16 @f(i16 %x) {
b0:
  %a = add i16 %x, 1
  br label %b1
b1:
  %i = phi i16 [ %a, %b0 ], [ %n, %b1 ]
  %n = add i16 %i, 1
  %c = call i1 @h(i16 %n)
  br i1 %c, label %b1, label %b2
b2:
  ret i16 %n
}
",
    );
    let before = get(Event::LoopExitScans);
    simplify(&mut module);
    assert_eq!(get(Event::LoopExitScans) - before, 0);
}

/// `FindFunctionBackedges`: the targets of back edges only, a join and a
/// forward edge into a loop are not headers, and a switch's cases are walked.
#[test]
fn test_loop_headers_are_the_targets_of_back_edges() {
    let module = crate::testing::parsed(
        "define i16 @f(i16 %x, i1 %c) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  switch i16 %x, label %b3 [
    i16 1, label %b4
  ]

b3:
  br i1 %c, label %b3, label %b4

b4:
  br i1 %c, label %b1, label %b5

b5:
  ret i16 0
}
",
    );
    let mut module = module;
    let (_, function) = module.function_mut("f").expect("@f");
    let blocks: Vec<_> = function.layout().to_vec();
    let headers = super::loop_headers(function);
    let got: Vec<usize> = (0..blocks.len()).filter(|&at| headers.contains(&blocks[at])).collect();
    assert_eq!(got, [1, 3]);
}

/// The preheader and the two exits of a rotated shellsort loop, each a jump
/// alone in a block with one predecessor, stayed through the LLVM-order
/// pipeline's SimplifyCFG (16 blocks in and out); the pin's, which asks only
/// the loop headers, leaves 9 (`opt -passes=simplifycfg`).
#[test]
fn test_a_jump_holding_a_loops_simple_form_goes_where_the_pin_removes_it() {
    let text = "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %b18

b18:
  %p = phi i16 [ 0, %b0 ], [ %next, %b35 ]
  %c = icmp ult i16 %p, %n
  br i1 %c, label %b35, label %b23

b23:
  br label %b24

b24:
  %q = phi i16 [ %p, %b23 ], [ %r, %b30 ]
  %r = sub i16 %q, 1
  %cc = icmp ugt i16 %r, %k
  br i1 %cc, label %b30, label %b34

b30:
  %d = icmp ult i16 %r, %n
  br i1 %d, label %b33, label %b24

b33:
  %split = phi i16 [ %r, %b30 ]
  br label %b35

b34:
  %split1 = phi i16 [ %q, %b24 ]
  br label %b35

b35:
  %m = phi i16 [ %split, %b33 ], [ %split1, %b34 ], [ %p, %b18 ]
  %next = add i16 %p, 1
  %e = icmp ult i16 %next, 8
  br i1 %e, label %b18, label %b40

b40:
  ret i16 %m
}
";
    let off = super::Options { loop_form_kept: false, ..Default::default() };
    check_with(
        &off,
        text,
        "define i16 @f(i16 %n, i16 %k) {
b0:
  br label %b18

b18:
  %p = phi i16 [ 0, %b0 ], [ %next, %b35 ]
  %c = icmp ult i16 %p, %n
  br i1 %c, label %b35, label %b24

b24:
  %q = phi i16 [ %r, %b30 ], [ %p, %b18 ]
  %r = sub i16 %q, 1
  %cc = icmp ugt i16 %r, %k
  br i1 %cc, label %b30, label %b35

b30:
  %d = icmp ult i16 %r, %n
  br i1 %d, label %b35, label %b24

b35:
  %m = phi i16 [ %p, %b18 ], [ %r, %b30 ], [ %q, %b24 ]
  %next = add i16 %p, 1
  %e = icmp ult i16 %next, 8
  br i1 %e, label %b18, label %b40

b40:
  ret i16 %m
}
",
        &[&[3, 0], &[10, 2], &[8, 3], &[6, 1]],
    );
}

/// A chain of compare blocks, each with one instruction to speculate, that
/// all leave to the same block: the pin folds each into the first one by one
/// (`requestResimplify` repeats the visit that folded, which deletes the block
/// it emptied). Ours moved on with the emptied block still a predecessor of the
/// next, so its bonus instruction counted twice and the chain stopped at pairs:
/// three `br i1` stayed where the pin leaves none (a QCport save-game test).
#[test]
fn test_a_chain_of_compare_blocks_folds_into_the_first_one_by_one() {
    let text = "define i16 @f(i16 %x) {
b0:
  %c = icmp ne i16 %x, 0
  br i1 %c, label %b1, label %bx

b1:
  %t1 = add i16 %x, 1
  %e1 = icmp ne i16 %t1, 4
  br i1 %e1, label %b2, label %bx

b2:
  %t2 = add i16 %x, 2
  %e2 = icmp ne i16 %t2, 7
  br i1 %e2, label %b3, label %bx

b3:
  %t3 = add i16 %x, 3
  %e3 = icmp ne i16 %t3, 9
  br i1 %e3, label %b4, label %bx

b4:
  %t4 = add i16 %x, 4
  %e4 = icmp ne i16 %t4, 11
  br i1 %e4, label %b5, label %bx

b5:
  ret i16 1

bx:
  ret i16 0
}
";
    let inputs: &[&[i128]] = &[&[0], &[1], &[3], &[5], &[7], &[9], &[100]];
    let mut module = parsed(text);
    let before = results(&module, inputs);
    simplify(&mut module);
    let after = printed(&module);
    assert!(!after.contains("br i1"), "{after}");
    assert_eq!(results(&module, inputs), before);
}

/// SimplifyCFG prices speculation by `TCK_CodeSize` in a `minsize` function
/// (`validateAndCostRequiredSelects`, `performBranchToCommonDestFolding`); the
/// kind was always `SizeAndLatency`. On the i486 only a divide is priced
/// differently (2 against 4), so the kind is checked where it is chosen.
#[test]
fn test_a_minsize_function_is_priced_by_code_size() {
    use llrm_mir::tti::CostKind;
    let text = "define void @f() {\nb0:\n  ret void\n}\n";
    let mut module = parsed(text);
    let (_, function) = module.function_mut("f").expect("@f");
    assert_eq!(crate::speculate::cost_kind(function), CostKind::SizeAndLatency);
    for attr in ["optsize", "minsize"] {
        let mut module = parsed(&text.replace("@f()", &format!("@f() {attr}")));
        let (_, function) = module.function_mut("f").expect("@f");
        let expected = if attr == "minsize" { CostKind::CodeSize } else { CostKind::SizeAndLatency };
        assert_eq!(crate::speculate::cost_kind(function), expected, "{attr}");
    }
}
