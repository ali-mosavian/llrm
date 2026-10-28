//! The expected text is the whole pass's result as the pinned `opt
//! -passes='simplifycfg<switch-range-to-icmp>'` prints it, up to value names:
//! later folds run on what the step under test leaves. Each body is MIR text
//! run by llrm-mir's interpreter before and after.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};

use crate::simplifycfg::simplified;
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
    let (context, function) = module.function_mut("f").expect("@f");
    assert_eq!(simplified(context, &spec, &mut declared, function), text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// A second `switch` on `%x` reached only by the default of the first kept
/// its case 1, which the first one took: the dead case went to `%cb`.
#[test]
fn test_a_case_the_predecessor_took_is_dead_in_its_default() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b1 [
    i16 1, label %ca
    i16 4, label %ca
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  store i16 9, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %cb
    i16 3, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %b1 [
    i16 1, label %ca
    i16 4, label %ca
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 4, %cd ], [ 3, %cc ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

b1:
  store i16 9, ptr @g
  %cond = icmp eq i16 %x, 3
  br i1 %cond, label %cc, label %cd

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[3], &[4], &[7]],
    );
}

/// The one constant that reaches the second `switch` is 1, so it is a jump to
/// `%cb`, and `%end`'s phi lost its inputs from `%cc` and the second `%cd`
/// edge.
#[test]
fn test_a_case_the_predecessor_sent_here_decides_the_switch() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %cd [
    i16 1, label %b1
    i16 2, label %ca
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  store i16 9, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %cb
    i16 3, label %cc
  ]

cb:
  store i16 2, ptr @g
  br label %end

cc:
  store i16 3, ptr @g
  br label %end

cd:
  store i16 4, ptr @g
  br label %end

end:
  %p = phi i16 [ 20, %cb ], [ 30, %cc ], [ 40, %cd ]
  ret i16 %p
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %cd [
    i16 1, label %b1
    i16 2, label %ca
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 20, %b1 ], [ 40, %cd ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

b1:
  store i16 9, ptr @g
  store i16 2, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// The conditional branch `x == 5` took 5 to `%ca`, so case 5 of the second
/// `switch` (reached by its false edge) is dead and `%cb` is unreachable.
#[test]
fn test_a_compare_that_took_a_constant_makes_that_case_dead() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 5
  br i1 %c, label %ca, label %b1

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  store i16 9, ptr @g
  switch i16 %x, label %cd [
    i16 5, label %cb
    i16 6, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 5
  br i1 %c, label %ca, label %b1

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 4, %cd ], [ 3, %cc ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

b1:
  store i16 9, ptr @g
  %cond = icmp eq i16 %x, 6
  br i1 %cond, label %cc, label %cd

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[4], &[5], &[6]],
    );
}

/// A pointer compared with `null` by the predecessor and cast to an integer
/// by the `switch` is one value: the `switch` case 0 is dead.
#[test]
fn test_a_null_compare_and_a_ptrtoint_switch_compare_one_value() {
    check(
        "@g = global i16 0

define i16 @f(i64 %x) {
b0:
  %p = inttoptr i64 %x to ptr
  %c = icmp eq ptr %p, null
  br i1 %c, label %ca, label %b1

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  store i16 9, ptr @g
  %i = ptrtoint ptr %p to i64
  switch i64 %i, label %cd [
    i64 0, label %cb
    i64 6, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i64 %x) {
b0:
  %p = inttoptr i64 %x to ptr
  %c = icmp eq ptr %p, null
  br i1 %c, label %ca, label %b1

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 4, %cd ], [ 3, %cc ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

b1:
  store i16 9, ptr @g
  %i = ptrtoint ptr %p to i64
  %cond = icmp eq i64 %i, 6
  br i1 %cond, label %cc, label %cd

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[0], &[6], &[7]],
    );
}

/// Cases 1 and 2 of `b0` go to `b1`, which is only a `switch` on `%x`: `b0`
/// stayed a switch that sends them to `b1` for a second switch.
#[test]
fn test_a_block_of_a_switch_folds_into_the_cases_that_reach_it() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %b1
    i16 2, label %b1
    i16 3, label %ca
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  switch i16 %x, label %ce [
    i16 1, label %cb
    i16 5, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

ce:
  store i16 5, ptr @g
  ret i16 5
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 3, label %ca
    i16 1, label %cb
    i16 2, label %ce
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 2, %cb ], [ 5, %ce ], [ 4, %cd ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cb:
  store i16 2, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

ce:
  store i16 5, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[5]],
    );
}

/// The default of `b0` is `b1`, a lone `switch` on `%x` of other cases: they
/// stayed two switches.
#[test]
fn test_a_switch_that_is_the_default_of_another_is_merged_into_it() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %b1 [
    i16 1, label %ca
    i16 2, label %cb
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  switch i16 %x, label %cd [
    i16 3, label %cc
    i16 4, label %ce
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

ce:
  store i16 5, ptr @g
  ret i16 5
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %ca
    i16 2, label %cb
    i16 3, label %cc
    i16 4, label %ce
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 2, %cb ], [ 3, %cc ], [ 4, %cd ], [ 5, %ce ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cb:
  store i16 2, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

ce:
  store i16 5, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[5]],
    );
}

/// `b0` and `b1` both go to `end`, whose phi takes 10 from one and 20 from the
/// other: the fold needs a block on `b1`'s edge, `end.fold.split`, or the phi
/// would lose a value.
#[test]
fn test_a_shared_successor_with_different_phi_inputs_is_split_first() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %b1 [
    i16 1, label %end
    i16 2, label %cb
  ]

b1:
  switch i16 %x, label %cd [
    i16 3, label %end
    i16 4, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

end:
  %p = phi i16 [ 10, %b0 ], [ 20, %b1 ]
  store i16 %p, ptr @g
  ret i16 %p
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %end
    i16 2, label %cb
    i16 3, label %end.fold.split
    i16 4, label %cc
  ]

common.ret:
  %common.op = phi i16 [ 2, %cb ], [ 3, %cc ], [ 4, %cd ], [ %p, %end ]
  ret i16 %common.op

cb:
  store i16 2, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

end.fold.split:
  br label %end

end:
  %p = phi i16 [ 10, %b0 ], [ 20, %end.fold.split ]
  store i16 %p, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[5]],
    );
}

/// `b0` branches on `%x == 1` to `b1`, a lone `switch` on `%x`: a branch and a
/// switch stayed.
#[test]
fn test_a_switch_that_is_the_false_edge_of_a_compare_is_merged_into_it() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %c = icmp eq i16 %x, 1
  br i1 %c, label %ca, label %b1

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  switch i16 %x, label %cd [
    i16 3, label %cc
    i16 4, label %ce
  ]

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

ce:
  store i16 5, ptr @g
  ret i16 5
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 1, label %ca
    i16 3, label %cc
    i16 4, label %ce
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 3, %cc ], [ 4, %cd ], [ 5, %ce ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

ce:
  store i16 5, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[3], &[4]],
    );
}

/// `b1` is its own default: the merged switch sends the rest to a block
/// `infloop` that loops, as `b1` did.
#[test]
fn test_a_switch_whose_default_is_itself_gets_a_block_that_loops() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %b1 [
    i16 1, label %ca
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  switch i16 %x, label %b1 [
    i16 3, label %cc
    i16 4, label %ce
  ]

cc:
  store i16 3, ptr @g
  ret i16 3

ce:
  store i16 5, ptr @g
  ret i16 5
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %infloop [
    i16 1, label %ca
    i16 3, label %cc
    i16 4, label %ce
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 3, %cc ], [ 5, %ce ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

ce:
  store i16 5, ptr @g
  br label %common.ret

infloop:
  br label %infloop
}
",
        &[&[1], &[3], &[4]],
    );
}

/// `b2` is only a `switch` on `%x` and is reached from `entry` and `q`: each of
/// them stayed a switch to `b2`.
#[test]
fn test_a_lone_switch_is_folded_into_each_of_its_predecessors() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
entry:
  store i16 8, ptr @g
  switch i16 %x, label %q [
    i16 1, label %b2
    i16 2, label %ca
  ]

q:
  store i16 7, ptr @g
  switch i16 %x, label %cd [
    i16 3, label %b2
    i16 4, label %cc
  ]

b2:
  switch i16 %x, label %ce [
    i16 1, label %cb
    i16 3, label %cf
  ]

ca:
  store i16 1, ptr @g
  ret i16 1

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

ce:
  store i16 5, ptr @g
  ret i16 5

cf:
  store i16 6, ptr @g
  ret i16 6
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
entry:
  store i16 8, ptr @g
  switch i16 %x, label %q [
    i16 2, label %ca
    i16 1, label %cb
  ]

q:
  store i16 7, ptr @g
  switch i16 %x, label %cd [
    i16 4, label %cc
    i16 3, label %cf
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 2, %cb ], [ 3, %cc ], [ 4, %cd ], [ 6, %cf ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cb:
  store i16 2, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

cf:
  store i16 6, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[5]],
    );
}

/// `b0` compares a pointer with `null` and `b1` switches on its `ptrtoint`:
/// they stayed a branch and a switch; the merged switch takes a fresh
/// `ptrtoint`, `%magicptr`.
#[test]
fn test_a_pointer_null_compare_and_a_switch_on_its_address_fold_through_magicptr() {
    check(
        "@g = global i16 0

define i16 @f(i64 %x) {
b0:
  %p = inttoptr i64 %x to ptr
  %i = ptrtoint ptr %p to i64
  %c = icmp eq ptr %p, null
  store i16 8, ptr @g
  br i1 %c, label %ca, label %b1

ca:
  store i16 1, ptr @g
  ret i16 1

b1:
  switch i64 %i, label %cd [
    i64 3, label %cc
    i64 4, label %ce
  ]

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4

ce:
  store i16 5, ptr @g
  ret i16 5
}
",
        "@g = global i16 0

define i16 @f(i64 %x) {
b0:
  %p = inttoptr i64 %x to ptr
  %i = ptrtoint ptr %p to i64
  store i16 8, ptr @g
  %magicptr = ptrtoint ptr %p to i64
  switch i64 %magicptr, label %cd [
    i64 0, label %ca
    i64 3, label %cc
    i64 4, label %ce
  ]

common.ret:
  %common.op = phi i16 [ 1, %ca ], [ 3, %cc ], [ 4, %cd ], [ 5, %ce ]
  ret i16 %common.op

ca:
  store i16 1, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret

ce:
  store i16 5, ptr @g
  br label %common.ret
}
",
        &[&[0], &[3], &[4], &[5]],
    );
}

/// `%key` is 3 for `%x == 5` and `%x` else: the switch stayed on `%key`. On
/// `%x` it needs a case 5 to `%cb`, where 3 goes.
#[test]
fn test_a_select_of_the_value_adds_a_case_that_goes_where_its_constant_did() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %cmp = icmp eq i16 %x, 5
  %key = select i1 %cmp, i16 3, i16 %x
  switch i16 %key, label %cd [
    i16 3, label %cb
    i16 7, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 3, label %cb
    i16 7, label %cc
    i16 5, label %cb
  ]

common.ret:
  %common.op = phi i16 [ 2, %cb ], [ 3, %cc ], [ 4, %cd ]
  ret i16 %common.op

cb:
  store i16 2, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[0], &[3], &[5], &[7], &[9]],
    );
}

/// `%key` is `%x` for `%x != 7` and 3 else: the switch stayed on `%key`. Case 7
/// now goes to `%cb` like 3, and `%cc` is gone.
#[test]
fn test_an_inverted_select_of_the_value_retargets_its_case() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %cmp = icmp ne i16 %x, 7
  %key = select i1 %cmp, i16 %x, i16 3
  switch i16 %key, label %cd [
    i16 3, label %cb
    i16 7, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 3, label %cb
    i16 7, label %cb
  ]

common.ret:
  %common.op = phi i16 [ 2, %cb ], [ 4, %cd ]
  ret i16 %common.op

cb:
  store i16 2, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[0], &[3], &[7], &[9]],
    );
}

/// `%key` is 3 or 7 by `%neg`: the switch on it stayed, with an edge to the
/// default left over.
#[test]
fn test_a_select_of_two_constants_is_a_branch_on_its_condition() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %neg = icmp slt i16 %x, 0
  %key = select i1 %neg, i16 3, i16 7
  switch i16 %key, label %cd [
    i16 3, label %cb
    i16 7, label %cc
    i16 9, label %cd
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %neg = icmp slt i16 %x, 0
  br i1 %neg, label %cb, label %cc

common.ret:
  %common.op = phi i16 [ 2, %cb ], [ 3, %cc ]
  ret i16 %common.op

cb:
  store i16 2, ptr @g
  br label %common.ret

cc:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[-1], &[0], &[1]],
    );
}

/// `%key` is 3 or 5, and 5 is no case: the branch goes to `%cb` and the
/// default.
#[test]
fn test_a_select_of_a_case_and_a_value_none_has_goes_to_the_default() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %neg = icmp slt i16 %x, 0
  %key = select i1 %neg, i16 3, i16 5
  switch i16 %key, label %cd [
    i16 3, label %cb
    i16 7, label %cc
  ]

cb:
  store i16 2, ptr @g
  ret i16 2

cc:
  store i16 3, ptr @g
  ret i16 3

cd:
  store i16 4, ptr @g
  ret i16 4
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %neg = icmp slt i16 %x, 0
  br i1 %neg, label %cb, label %cd

common.ret:
  %common.op = phi i16 [ 2, %cb ], [ 4, %cd ]
  ret i16 %common.op

cb:
  store i16 2, ptr @g
  br label %common.ret

cd:
  store i16 4, ptr @g
  br label %common.ret
}
",
        &[&[-1], &[0]],
    );
}

/// Case 7 moves to `%end`, which already has an edge from `b0` through case 3:
/// its phi needs a second input from `b0`, and loses the one it had from `%cc`.
#[test]
fn test_a_retargeted_case_gives_the_new_destination_a_phi_input_per_edge() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  %cmp = icmp eq i16 %x, 7
  %key = select i1 %cmp, i16 3, i16 %x
  switch i16 %key, label %cd [
    i16 3, label %end
    i16 7, label %cc
  ]

cc:
  store i16 3, ptr @g
  br label %end

cd:
  store i16 4, ptr @g
  br label %end

end:
  %p = phi i16 [ 20, %b0 ], [ 30, %cc ], [ 40, %cd ]
  store i16 %p, ptr @g
  ret i16 %p
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  store i16 8, ptr @g
  switch i16 %x, label %cd [
    i16 3, label %end
    i16 7, label %end
  ]

cd:
  store i16 4, ptr @g
  br label %end

end:
  %p = phi i16 [ 20, %b0 ], [ 20, %b0 ], [ 40, %cd ]
  store i16 %p, ptr @g
  ret i16 %p
}
",
        &[&[0], &[3], &[7], &[9]],
    );
}

/// A conditional branch on `%x == 1` reached only from a branch on the same
/// compare, taking its false arm: its own true arm is dead (`opt` leaves one
/// branch and drops `%u`). `simplifyCondBranch`'s call of
/// `simplifyEqualityComparisonWithOnlyPredecessor` was not made.
#[test]
fn test_a_branch_on_the_value_its_only_predecessor_compared_is_a_jump() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %t, label %b1

b1:
  %d = icmp eq i16 %x, 1
  br i1 %d, label %u, label %v

t:
  store i16 1, ptr @g
  ret i16 1

u:
  store i16 2, ptr @g
  ret i16 2

v:
  store i16 3, ptr @g
  ret i16 3
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %t, label %v

common.ret:
  %common.op = phi i16 [ 1, %t ], [ 3, %v ]
  ret i16 %common.op

t:
  store i16 1, ptr @g
  br label %common.ret

v:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[7]],
    );
}

/// A branch block of only its compare folds into the predecessor that compares
/// the same value, which becomes a `switch`
/// (`foldValueComparisonIntoPredecessors` for a `br`).
#[test]
fn test_a_branch_of_only_its_compare_folds_into_the_predecessors_branch() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %t, label %b1

b1:
  %d = icmp eq i16 %x, 7
  br i1 %d, label %u, label %v

t:
  store i16 1, ptr @g
  ret i16 1

u:
  store i16 2, ptr @g
  ret i16 2

v:
  store i16 3, ptr @g
  ret i16 3
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %v [
    i16 1, label %t
    i16 7, label %u
  ]

common.ret:
  %common.op = phi i16 [ 1, %t ], [ 2, %u ], [ 3, %v ]
  ret i16 %common.op

t:
  store i16 1, ptr @g
  br label %common.ret

u:
  store i16 2, ptr @g
  br label %common.ret

v:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[0], &[1], &[7]],
    );
}

/// The same with `ne`: the compare's sense picks which arm is the case.
#[test]
fn test_a_ne_branch_reached_by_the_equal_value_is_a_jump_to_the_other_arm() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp ne i16 %x, 3
  br i1 %c, label %b1, label %t

b1:
  %d = icmp eq i16 %x, 3
  br i1 %d, label %u, label %v

t:
  store i16 1, ptr @g
  ret i16 1

u:
  store i16 2, ptr @g
  ret i16 2

v:
  store i16 3, ptr @g
  ret i16 3
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c = icmp ne i16 %x, 3
  br i1 %c, label %v, label %t

common.ret:
  %common.op = phi i16 [ 1, %t ], [ 3, %v ]
  ret i16 %common.op

t:
  store i16 1, ptr @g
  br label %common.ret

v:
  store i16 3, ptr @g
  br label %common.ret
}
",
        &[&[0], &[3], &[7]],
    );
}

/// QCport `_pl_trace_box`: a chain `v == 0`, `v == 1`, `v == 2` on one value,
/// each arm a store joining one block. The pin makes one `switch`; ours kept
/// three `br i1` (5 more branches in the function, +73 B at -Os).
#[test]
fn test_a_chain_of_equality_branches_on_one_value_is_one_switch() {
    check(
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  %c0 = icmp eq i16 %x, 0
  br i1 %c0, label %a, label %b

b:
  %c1 = icmp eq i16 %x, 1
  br i1 %c1, label %c, label %d

d:
  %c2 = icmp eq i16 %x, 2
  br i1 %c2, label %e, label %join

a:
  store i16 10, ptr @g
  br label %join

c:
  store i16 11, ptr @g
  br label %join

e:
  store i16 12, ptr @g
  br label %join

join:
  %r = load i16, ptr @g
  ret i16 %r
}
",
        "@g = global i16 0

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %join [
    i16 0, label %a
    i16 1, label %c
    i16 2, label %e
  ]

a:
  store i16 10, ptr @g
  br label %join

c:
  store i16 11, ptr @g
  br label %join

e:
  store i16 12, ptr @g
  br label %join

join:
  %r = load i16, ptr @g
  ret i16 %r
}
",
        &[&[0], &[1], &[2], &[3]],
    );
}

/// hanoi (D10 copies): compares `x == 0, 1, 2` into one block with a phi of
/// constants. The pin makes one `switch` (`out.fold.split` blocks); ours kept
/// every compare.
#[test]
fn test_a_chain_of_equality_branches_into_one_phi_is_one_switch() {
    check(
        "define i32 @f(i32 %x) {
b0:
  %a = icmp eq i32 %x, 0
  br i1 %a, label %out, label %b1

b1:
  %b = icmp eq i32 %x, 1
  br i1 %b, label %out, label %b2

b2:
  %c = icmp eq i32 %x, 2
  br i1 %c, label %out, label %b3

b3:
  %d = mul i32 %x, 7
  br label %out

out:
  %r = phi i32 [ 0, %b0 ], [ 1, %b1 ], [ 3, %b2 ], [ %d, %b3 ]
  ret i32 %r
}
",
        "define i32 @f(i32 %x) {
b0:
  switch i32 %x, label %b3 [
    i32 0, label %out
    i32 1, label %out.fold.split
    i32 2, label %out.fold.split1
  ]

b3:
  %d = mul i32 %x, 7
  br label %out

out.fold.split:
  br label %out

out.fold.split1:
  br label %out

out:
  %r = phi i32 [ 0, %b0 ], [ %d, %b3 ], [ 1, %out.fold.split ], [ 3, %out.fold.split1 ]
  ret i32 %r
}
",
        &[&[0], &[1], &[2], &[3], &[9]],
    );
}
