//! Each body is MIR text run by llrm-mir's interpreter before and after.

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use super::simplified;
use crate::testing::{parsed, printed, results};

/// @f simplified; whether anything changed.
fn simplify(module: &mut Module) -> bool {
    let outer = Outer::of(module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified(context, outer.callees(), function)
}

/// `text` simplified is `expected`, and computes what it did on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    assert_eq!(simplify(&mut module), text != expected, "{text}");
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
  br i1 %cond, label %b1, label %b2

b1:
  ret i16 10

b2:
  ret i16 20
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

#[test]
fn test_a_branch_on_a_value_is_left_alone() {
    let text = "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 1
  br i1 %c, label %b1, label %b2

b1:
  ret i16 10

b2:
  ret i16 20
}
";
    check(text, text, &[&[1], &[2]]);
}

/// The jump that closes a loop keeps its `!llvm.loop`.
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

/// A chain of n folded branches, each leaving a phi of one value, simplified.
fn chain_time(n: usize) -> std::time::Duration {
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
    (0..5)
        .map(|_| {
            let mut module = parsed(&text);
            let start = std::time::Instant::now();
            simplify(&mut module);
            start.elapsed()
        })
        .min()
        .unwrap()
}

/// The fold loop and the phi cleanup after it took time linear in the
/// function: 4x the blocks must cost well under the 16x of a quadratic one.
#[test]
fn test_simplifying_a_chain_of_folds_is_linear_in_its_length() {
    let small = chain_time(400);
    let large = chain_time(1600);
    assert!(large < small * 9, "400 blocks {small:?}, 1600 blocks {large:?}: slope above 1.6");
}
