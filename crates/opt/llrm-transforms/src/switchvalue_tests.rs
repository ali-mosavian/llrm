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
