//! The scalar part of `GVNPass` (`gvn::pass`, `gvn::table`): value numbering of
//! arithmetic, comparisons, GEPs and calls that touch no memory, equality
//! propagated from branches, switches and assumes, and dead blocks. Each case
//! runs the interpreter before and after.

use llrm_mir::module::Module;
use llrm_mir::passes::Outer;

use crate::gvn::pass::run_impl;
use crate::testing::{parsed, printed, results};

const XY: &[&[i128]] = &[&[0, 0], &[3, 5], &[-7, 2], &[0x7fff, 1]];

/// `text`'s @f after the scalar pass: printed, and whether it changed. The pass
/// again changes nothing; what @f returns for `inputs` stays.
fn numbered(
    text: &str,
    inputs: &[&[i128]],
) -> (String, bool) {
    numbered_with(text, inputs, true)
}

/// As `numbered`; `settles` says the pass leaves nothing for a second run to
/// change (a branch on a constant is found dead on every run).
fn numbered_with(
    text: &str,
    inputs: &[&[i128]],
    settles: bool,
) -> (String, bool) {
    let before = parsed(text);
    let mut module: Module = before.clone();
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = run_impl(context, &layout, function, &outer);
    let printed = printed(&module);
    assert_eq!(results(&module, inputs), results(&before, inputs), "{printed}");
    let (context, function) = module.function_mut("f").expect("@f");
    assert!(!settles || !run_impl(context, &layout, function, &outer), "a second run changes nothing:\n{printed}");
    (printed, changed)
}

fn kept(text: &str) {
    let (printed_text, changed) = numbered(text, &[]);
    assert!(!changed, "{printed_text}");
    assert_eq!(printed_text, printed(&parsed(text)));
}

#[test]
fn test_a_repeated_expression_is_computed_once() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = mul i16 %x, %y
  %b = mul i16 %x, %y
  %r = sub i16 %a, %b
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    // The difference of a value and itself is simplified on the way.
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_operands_in_either_order_are_one_commutative_expression() {
    for op in ["add", "xor", "mul", "and", "or"] {
        let (text, changed) = numbered(
            &format!(
                "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext i16 %a to i32
  %t = zext i16 %b to i32
  %r = sub i32 %s, %t
  ret i32 %r
}}
"
            ),
            XY,
        );
        assert!(changed, "{op}");
        assert!(text.contains("ret i32 0"), "{op}: {text}");
    }
}

/// `icmp slt x, y` and `icmp sgt y, x` are one comparison: LLVM sorts the
/// operand numbers and swaps the predicate with them.
#[test]
fn test_a_compare_with_its_operands_and_predicate_swapped_is_one() {
    let (text, changed) = numbered(
        "define i32 @f(i16 %x, i16 %y) {
b0:
  %a = icmp slt i16 %x, %y
  %b = icmp sgt i16 %y, %x
  %s = zext i1 %a to i32
  %t = zext i1 %b to i32
  %r = sub i32 %s, %t
  ret i32 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("ret i32 0"), "{text}");
}

#[test]
fn test_an_ordered_expression_with_its_operands_swapped_stays() {
    for op in ["sub", "shl"] {
        kept(&format!(
            "define i32 @f(i16 %x, i16 %y) {{
b0:
  %a = {op} i16 %x, %y
  %b = {op} i16 %y, %x
  %s = zext i16 %a to i32
  %t = zext i16 %b to i32
  %r = add i32 %s, %t
  ret i32 %r
}}
"
        ));
    }
}

/// LLVM's key has no IR flags and a merge keeps the flags both have; here the
/// flags are in the key (DEVIATE 16). Merged, `%a` lost the `nsw` a later pass
/// reads: x_mergesort -O2 +5.9% memory operands, -Os +21%.
#[test]
fn test_expressions_differing_in_flags_stay_two() {
    kept(
        "define i16 @f(i16 %x) {
b0:
  %a = add nsw i16 %x, 1
  %b = add i16 %x, 1
  %r = xor i16 %a, %b
  ret i16 %r
}
",
    );
}

#[test]
fn test_expressions_differing_in_an_operand_or_type_stay() {
    for (first, second) in
        [("%a = add i16 %x, 1", "%b = add i16 %x, 2"), ("%a = sext i8 %n to i16", "%b = zext i8 %n to i16")]
    {
        kept(&format!(
            "define i16 @f(i16 %x, i8 %n) {{
b0:
  {first}
  {second}
  %r = add i16 %a, %b
  ret i16 %r
}}
"
        ));
    }
    kept(
        "define i32 @f(i8 %n) {
b0:
  %a = zext i8 %n to i16
  %b = zext i8 %n to i32
  %w = zext i16 %a to i32
  %r = add i32 %w, %b
  ret i32 %r
}
",
    );
}

/// Addresses that differ in their type encoding but add the same bytes are one
/// (`createGEPExpr`).
#[test]
fn test_geps_adding_the_same_bytes_are_one() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %n) {
b0:
  %p = inttoptr i16 %n to ptr
  %a = getelementptr i16, ptr %p, i16 2
  %b = getelementptr i8, ptr %p, i16 4
  %x = ptrtoint ptr %a to i16
  %y = ptrtoint ptr %b to i16
  %r = sub i16 %x, %y
  ret i16 %r
}
",
        &[&[0], &[100]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_geps_adding_different_bytes_stay() {
    kept(
        "define i16 @f(i16 %n) {
b0:
  %p = inttoptr i16 %n to ptr
  %a = getelementptr i16, ptr %p, i16 2
  %b = getelementptr i8, ptr %p, i16 5
  %x = ptrtoint ptr %a to i16
  %y = ptrtoint ptr %b to i16
  %r = add i16 %x, %y
  ret i16 %r
}
",
    );
}

#[test]
fn test_a_dominated_repeat_reads_the_dominating_value() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = and i16 %x, %y
  %c = icmp eq i16 %x, 0
  br i1 %c, label %b1, label %b2

b1:
  %b = and i16 %y, %x
  ret i16 %b

b2:
  ret i16 %a
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("b1:\n  ret i16 0\n") || text.contains("b1:\n  ret i16 %a\n"), "{text}");
}

/// Neither arm runs before the other.
#[test]
fn test_a_repeat_in_a_sibling_block_is_kept() {
    kept(
        "define i16 @f(i16 %x, i16 %y, i16 %z) {
b0:
  %c = icmp eq i16 %z, 0
  br i1 %c, label %b1, label %b2

b1:
  %a = or i16 %x, %y
  ret i16 %a

b2:
  %b = or i16 %x, %y
  ret i16 %b
}
",
    );
}

#[test]
fn test_a_branch_condition_is_known_in_each_arm() {
    let (text, changed) = numbered(
        "define i16 @f(i1 %c, i16 %x) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  %t = zext i1 %c to i16
  ret i16 %t

b2:
  %u = zext i1 %c to i16
  ret i16 %u
}
",
        &[&[0, 0], &[1, 0]],
    );
    assert!(changed);
    assert!(text.contains("zext i1 true") || text.contains("ret i16 1"), "{text}");
}

/// `x == 5` taken: the compare is true in the arm, and `x` stays (DEVIATE 17:
/// replacing it by 5 made matmul -O2 +5.7% code bytes on Nib).
#[test]
fn test_an_equality_taken_leaves_the_compared_operand() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 5
  br i1 %c, label %b1, label %b2

b1:
  %d = icmp eq i16 %x, 5
  %e = zext i1 %d to i16
  %y = add i16 %x, %x
  %r = add i16 %y, %e
  ret i16 %r

b2:
  ret i16 0
}
",
        &[&[5], &[6]],
    );
    assert!(changed);
    assert!(text.contains("%y = add i16 %x, %x") && text.contains("add i16 %y, 1"), "{text}");
}

/// The case value is the switch value in the case's block, when one edge
/// reaches it.
#[test]
fn test_a_switch_case_value_replaces_the_variable_in_its_block() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 3, label %c3
    i16 4, label %c4
  ]

c3:
  %a = add i16 %x, 1
  ret i16 %a

c4:
  %b = add i16 %x, 1
  ret i16 %b

d:
  ret i16 0
}
",
        &[&[3], &[4], &[9]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 4") && text.contains("ret i16 5"), "{text}");
}

#[test]
fn test_an_assume_makes_its_condition_true_below_it() {
    let (text, changed) = numbered(
        "declare void @llvm.assume(i1)

define i16 @f(i16 %x) {
b0:
  %c = icmp eq i16 %x, 7
  call void @llvm.assume(i1 %c)
  %d = zext i1 %c to i16
  ret i16 %d
}
",
        &[&[7]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 1"), "{text}");
}

/// A branch on a constant makes its other arm dead; the phi it feeds takes
/// poison from it.
#[test]
fn test_a_branch_on_a_constant_poisons_the_dead_arms_phi_input() {
    let (text, changed) = numbered_with(
        "define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ %x, %b2 ]
  ret i16 %p
}
",
        &[&[3]],
        false,
    );
    assert!(changed);
    // The poisoned phi is then the constant of the live arm.
    assert!(text.contains("ret i16 1"), "{text}");
}

#[test]
fn test_two_identical_phis_are_one() {
    let (text, changed) = numbered(
        "define i16 @f(i1 %c, i16 %x, i16 %y) {
b0:
  br i1 %c, label %b1, label %b2

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %q = phi i16 [ %x, %b1 ], [ %y, %b2 ]
  %r = sub i16 %p, %q
  ret i16 %r
}
",
        &[&[0, 1, 2], &[1, 1, 2]],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_two_calls_that_touch_no_memory_are_one() {
    let (text, changed) = numbered(
        "declare i16 @pure(i16) readnone

define i16 @f(i16 %x) {
b0:
  %a = call i16 @pure(i16 %x)
  %b = call i16 @pure(i16 %x)
  %r = sub i16 %a, %b
  ret i16 %r
}
",
        &[],
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

#[test]
fn test_two_calls_that_read_memory_stay() {
    kept(
        "declare i16 @peek(i16) readonly

define i16 @f(i16 %x) {
b0:
  %a = call i16 @peek(i16 %x)
  %b = call i16 @peek(i16 %x)
  %r = sub i16 %a, %b
  ret i16 %r
}
",
    );
}

/// The wrapped result of `add.with.overflow` is the add.
#[test]
fn test_the_sum_of_an_overflow_intrinsic_is_the_add() {
    let (text, changed) = numbered(
        "declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16)

define i16 @f(i16 %x, i16 %y) {
b0:
  %o = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %x, i16 %y)
  %s = extractvalue { i16, i1 } %o, 0
  %a = add i16 %x, %y
  %r = sub i16 %s, %a
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("ret i16 0"), "{text}");
}

/// A phi of one input is that input.
#[test]
fn test_a_phi_with_one_input_is_its_input() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %j = add i16 %i, 1
  %c = icmp ult i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  %out = phi i16 [ %j, %b1 ]
  ret i16 %out
}
",
        &[&[5], &[9]],
    );
    assert!(changed && !text.contains("%out") && text.contains("ret i16 %j"), "{text}");
}

/// matrix printed T=0 for T=380 after CSE deleted a zero still named by its
/// loop phi.
#[test]
fn test_cse_replaces_phi_uses_of_a_deleted_initializer() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %first = add i16 %x, 7
  br label %b1

b1:
  %second = add i16 %x, 7
  br label %b2

b2:
  %result = phi i16 [ %second, %b1 ]
  %r = add i16 %result, %second
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    // The blocks merge, the phi folds, and no use names the deleted add.
    assert!(!text.contains("%second") && !text.contains("%result"), "{text}");
    assert!(text.contains("%r = add i16 %first, %first"), "{text}");
}

/// Descriptor addresses encoded as zero must not become the same value: the
/// casts fold to two constant expressions (`ConstantFoldCastOperand`), not one.
#[test]
fn test_cse_keeps_distinct_linker_addresses() {
    let (text, _) = numbered(
        "@a = global i16 0
@b = global i16 0

define i16 @f() {
b0:
  %first = ptrtoint ptr @a to i16
  %second = ptrtoint ptr @b to i16
  %r = add i16 %first, %second
  ret i16 %r
}
",
        &[],
    );
    assert!(text.contains("add i16 ptrtoint (ptr @a to i16), ptrtoint (ptr @b to i16)"), "{text}");
}

/// `ptrtoint ptr null` is the number 0 (ConstantFolding casts a null pointer to
/// a null), so `a + 0` is `a`; `@a`'s address stays what it was.
#[test]
fn test_the_address_of_null_is_the_number_zero() {
    let (text, _) = numbered(
        "@a = global i16 0

define i16 @f() {
b0:
  %first = ptrtoint ptr @a to i16
  %second = ptrtoint ptr null to i16
  %r = add i16 %first, %second
  ret i16 %r
}
",
        &[],
    );
    assert!(!text.contains("add i16") && text.contains("ptrtoint (ptr @a to i16)"), "{text}");
}

#[test]
fn test_cse_reuses_one_frame_object_address() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %frame = alloca [66 x i16]
  %first = getelementptr i8, ptr %frame, i16 4
  %duplicate = getelementptr i8, ptr %frame, i16 4
  store i16 %x, ptr %first
  %r = load i16, ptr %duplicate
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(!text.contains("%duplicate"), "{text}");
    // The load of the stored bytes is the stored value.
    assert!(text.contains("ret i16 %x"), "{text}");
}

/// An address took its cell for a read, so no two addresses of one descriptor
/// were ever one value and no load through them was shared.
#[test]
fn test_cse_reuses_one_cell_address() {
    let (text, changed) = numbered(
        "@descriptor = global [20 x i8] zeroinitializer

define i16 @f(i16 %x, i16 %y) {
b0:
  %first = getelementptr i8, ptr @descriptor, i16 18
  %duplicate = getelementptr i8, ptr @descriptor, i16 18
  store i16 %x, ptr %first
  %r = load i16, ptr %duplicate
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(!text.contains("%duplicate"), "{text}");
}

/// lngmix under SROA refused 0x0071: "mov ... defines [v24_1] through no
/// operand": the third divide was served the second's quotient, which the
/// second no longer computed.
#[test]
fn test_a_third_equal_divide_reads_the_answer_the_first_computed() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %q0 = sdiv i16 %x, %y
  %r0 = srem i16 %x, %y
  %q1 = sdiv i16 %x, %y
  %r1 = srem i16 %x, %y
  %q2 = sdiv i16 %x, %y
  %r2 = srem i16 %x, %y
  %a = add i16 %r1, %q2
  %b = add i16 %a, %q0
  ret i16 %b
}
",
        &[&[7, 2], &[-9, 4], &[100, -3]],
    );
    assert!(changed);
    assert!(text.contains("  %a = add i16 %r0, %q0\n  %b = add i16 %a, %q0\n"), "{text}");
    assert_eq!(text.matches("sdiv").count() + text.matches("srem").count(), 2, "{text}");
}

#[test]
fn test_floating_arithmetic_is_one_expression() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %u = sitofp i16 %x to double
  %v = sitofp i16 %x to double
  %a = fdiv double %u, 3.0
  %b = fdiv double %v, 3.0
  %s = fadd double %a, %b
  %r = fptosi double %s to i16
  ret i16 %r
}
",
        XY,
    );
    assert!(changed);
    assert!(text.contains("%s = fadd double %a, %a"), "{text}");
}

/// A loop's invariant phi (`phi [%x, entry], [itself, latch]`) is `%x`, as
/// `simplifyInstruction` has it.
#[test]
fn test_a_loops_invariant_phi_is_its_input() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %n, i16 %x) {
b0:
  %outside = mul i16 %x, 64
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %j, %b1 ]
  %inv = phi i16 [ %x, %b0 ], [ %inv, %b1 ]
  %inside = mul i16 %inv, 64
  %j = add i16 %i, %inside
  %c = icmp ult i16 %j, %n
  br i1 %c, label %b1, label %b2

b2:
  %r = add i16 %j, %outside
  ret i16 %r
}
",
        &[&[5, 1], &[9, 2]],
    );
    assert!(changed && !text.contains("%inv = phi") && text.contains("mul i16 %x, 64"), "{text}");
}

/// A GEP of offset zero stays (DEVIATE 18): simplified to its pointer, the
/// BASIC runtime compiled at -Os grew in two objects (nhstutil +10 bytes, play
/// +9) and crc, jpeg and stft on `bq32` ran +0.1..0.55% instructions.
#[test]
fn test_a_gep_of_offset_zero_is_kept() {
    kept(
        "define i16 @f(i16 %n) {
b0:
  %p = inttoptr i16 %n to ptr
  %q = getelementptr i8, ptr %p, i16 0
  %v = load i16, ptr %q
  ret i16 %v
}
",
    );
}

/// The tables were emptied three times a run (at the start of each iteration
/// and again at the end); `cleanupGlobalSets` empties them once per iteration
/// over the function: a body that settles in one iteration clears once.
#[test]
fn test_a_run_that_settles_in_one_iteration_clears_the_tables_once() {
    let text = "define i16 @f(i16 %x, i16 %y) {
b0:
  %a = add i16 %x, %y
  %b = mul i16 %a, %y
  ret i16 %b
}
";
    let mut module = parsed(text);
    let layout = llrm_analysis::testing::layout(&module);
    let outer = Outer::of(&module, None);
    let (context, function) = module.function_mut("f").expect("@f");
    let before = crate::gvn::table::clears();
    assert!(!run_impl(context, &layout, function, &outer));
    assert_eq!(crate::gvn::table::clears() - before, 1);
}

/// A phi of a constant and poison is the constant. The pass asked no phi of
/// `simplifyInstruction`, so matmul at -O3 kept 44 such phis; the `sext` and
/// `mul` of each stayed and made the copies of the unrolled `j` loop too long
/// for memdep's scan: 8 `a[i][k]` were loaded again (16 `movsx` for the pin's
/// 8).
#[test]
fn test_a_phi_of_a_constant_and_poison_is_the_constant() {
    let (text, changed) = numbered(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp slt i16 %x, 1000
  br i1 %c, label %b2, label %b1

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ poison, %b1 ], [ 2, %b2 ]
  %r = add i16 %p, %y
  ret i16 %r
}
",
        &[&[3, 5], &[-7, 2]],
    );
    assert!(changed && !text.contains("phi") && text.contains("add i16 2, %y"), "{text}");
}

/// Two different constants stay a phi.
#[test]
fn test_a_phi_of_two_constants_stays() {
    kept(
        "define i16 @f(i16 %x, i16 %y) {
b0:
  %c = icmp slt i16 %x, 3
  br i1 %c, label %b2, label %b1

b1:
  br label %b3

b2:
  br label %b3

b3:
  %p = phi i16 [ 1, %b1 ], [ 2, %b2 ]
  %r = add i16 %p, %y
  ret i16 %r
}
",
    );
}
