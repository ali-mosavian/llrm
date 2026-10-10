//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong value shows as a wrong answer; what is asserted of the text is that
//! the phi of constants is gone, or kept where the branch is the cheaper.

use llrm_analysis::testing::DOS;

use super::PhiOpt;
use crate::profit::OperationCosts;
use crate::testing::{Tuned, managed_on, parsed, results};

/// A machine whose branch costs more than the arithmetic: a deep pipeline's.
fn deep() -> Tuned {
    Tuned {
        costs: OperationCosts {
            branch: 6,
            r#move: 1,
            extend: 1,
            set: 1,
            add: 1,
            shift: 1,
            multiply: 10,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A 486's prices, in clocks (`setcc` 4, `movzx` 3, `jcc` 3, a multiply 26).
fn i486() -> Tuned {
    Tuned {
        costs: OperationCosts {
            branch: 3,
            r#move: 1,
            extend: 3,
            set: 4,
            add: 1,
            shift: 2,
            multiply: 26,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn optimized(
    text: &str,
    machine: Tuned,
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed_on(&mut module, PhiOpt { size: false }, machine);
    let inputs: &[&[i128]] = &[&[0], &[1], &[2], &[3], &[127], &[128], &[200], &[255]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// `c < 128 ? 2 : 0` was a branch around two empty blocks into a phi of 2
/// and 0: x_switch's states then stayed branches that jump threading copied
/// 13 times (1173 B, gcc's 903).
#[test]
fn test_two_or_zero_on_a_condition_is_a_shift() {
    let after = optimized(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp slt i32 %x, 128
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %s = phi i32 [ 2, %t ], [ 0, %e ]
  ret i32 %s
}
",
        deep(),
    );
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("shl"), "{after}");
}

/// `c & 1 ? 3 : 0` is the condition as a mask of 3, as gcc's
/// `-cond & 3`.
#[test]
fn test_three_or_zero_on_a_condition_is_a_mask() {
    let after = optimized(
        "define i32 @f(i32 %x) {
b0:
  %a = and i32 %x, 1
  %c = icmp ne i32 %a, 0
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %s = phi i32 [ 3, %t ], [ 0, %e ]
  ret i32 %s
}
",
        deep(),
    );
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("sext") && after.contains("and i32"), "{after}");
}

/// One and zero on a triangle (the false side is the join) is the condition
/// extended; the other way round is its complement's.
#[test]
fn test_one_or_zero_on_a_triangle_is_the_condition_extended() {
    for (yes, no) in [(1, 0), (0, 1), (5, 4), (4, 5)] {
        let after = optimized(
            &format!(
                "define i32 @f(i32 %x) {{
b0:
  %c = icmp slt i32 %x, 128
  br i1 %c, label %t, label %j

t:
  br label %j

j:
  %s = phi i32 [ {yes}, %t ], [ {no}, %b0 ]
  ret i32 %s
}}
"
            ),
            deep(),
        );
        assert!(!after.contains("phi"), "{yes}/{no}: {after}");
    }
}

/// A 486 holds a condition in `setcc` and `movzx`, 7 clocks before the
/// arithmetic, against a branch of 3: priced by the extend alone, the
/// conversion made x_switch 59% slower (45,159 clocks to 71,757) for 22% fewer
/// bytes.
#[test]
fn test_a_486_keeps_the_branch_its_setcc_costs_more_than() {
    let after = optimized(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp slt i32 %x, 128
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %s = phi i32 [ 2, %t ], [ 0, %e ]
  ret i32 %s
}
",
        i486(),
    );
    assert!(after.contains("phi"), "{after}");
}

/// Where the arithmetic costs more than the branch it replaces, the branch
/// stays: a target with a free branch and a dear extend.
#[test]
fn test_a_branch_that_costs_less_than_the_arithmetic_stays() {
    let machine = Tuned {
        costs: OperationCosts { branch: 0, r#move: 0, extend: 9, add: 1, shift: 2, ..Default::default() },
        ..Default::default()
    };
    let after = optimized(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp slt i32 %x, 128
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %s = phi i32 [ 2, %t ], [ 0, %e ]
  ret i32 %s
}
",
        machine,
    );
    assert!(after.contains("phi"), "{after}");
}

/// `c > 200 ? 4 : 1` is three times the condition and one where the target
/// makes the multiply of one `lea`: with the mask and the add it was 6 clocks
/// against the branch's 5 and stayed a branch, a state jump threading copied.
#[test]
fn test_four_or_one_on_a_condition_is_a_cheap_multiply() {
    let mut machine = deep();
    machine.multiplies.insert(3, 1);
    let after = optimized(
        "define i32 @f(i32 %x) {
b0:
  %c = icmp sgt i32 %x, 200
  br i1 %c, label %t, label %e

t:
  br label %j

e:
  br label %j

j:
  %s = phi i32 [ 4, %t ], [ 1, %e ]
  ret i32 %s
}
",
        machine,
    );
    assert!(!after.contains("phi"), "{after}");
    assert!(after.contains("mul i32"), "{after}");
}

/// A 486's `setcc` and `movzx` come to more than its branch and copy, so the
/// body is not walked; a deep pipeline's do not.
#[test]
fn test_only_a_target_whose_arithmetic_can_win_walks_the_body() {
    assert!(!super::can_win(&i486().costs));
    assert!(super::can_win(&deep().costs));
}
