//! Each body is MIR text run by llrm-mir's interpreter before and after; the
//! expected text is what the pinned `opt -mtriple=i486-linux
//! -passes='simplifycfg<switch-to-lookup;switch-to-arithmetic>'` prints.

use llrm_mir::datalayout::DataLayout;
use llrm_mir::passes::{Declared, Outer};
use llrm_x86::tti::X86;

use crate::simplifycfg::{Options, simplified_with};
use crate::testing::{parsed, printed, results};
use crate::twoentryphi::Speculation;

/// `text` simplified at the late slot is `expected`, and computes what it did
/// on `inputs`.
fn check(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("e-p:32:32-i64:32-n8:16:32").expect("a layout");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let spec = Speculation { tti: &tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let changed = simplified_with(&Options::L1438, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("the declarations are placed");
    assert_eq!(changed, text != expected, "{text}");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// `switch ty %x` with a block per case that jumps to `end`, whose phi holds
/// the case's result; `default` goes to `d` and is the phi's input there.
fn diamond(
    ty: &str,
    cases: &[(i128, i128)],
    default: i128,
) -> String {
    let mut text = format!("define {ty} @f({ty} %x) {{\nb0:\n  switch {ty} %x, label %d [\n");
    for (at, (case, _)) in cases.iter().enumerate() {
        text += &format!("    {ty} {case}, label %c{at}\n");
    }
    text += "  ]\n\n";
    for at in 0..cases.len() {
        text += &format!("c{at}:\n  br label %end\n\n");
    }
    text += &format!("d:\n  br label %end\n\nend:\n  %p = phi {ty} ");
    for (at, (_, result)) in cases.iter().enumerate() {
        text += &format!("[ {result}, %c{at} ], ");
    }
    text += &format!("[ {default}, %d ]\n  ret {ty} %p\n}}\n");
    text
}

/// Cases 10, 13, 16, 19 and a default 0 stayed a switch and a phi of five
/// constants.
#[test]
fn test_cases_in_a_line_are_a_multiply_and_an_add() {
    check(
        &diamond("i16", &[(0, 10), (1, 13), (2, 16), (3, 19)], 0),
        "define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 4
  %switch.idx.mult = mul nsw i16 %x, 3
  %switch.offset = add nsw i16 %switch.idx.mult, 10
  %1 = select i1 %0, i16 %switch.offset, i16 0
  ret i16 %1
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[-1]],
    );
}

/// Cases 5 to 8 with results 100 down to 70 kept their switch: the index is
/// `x - 5` and the step is -10.
#[test]
fn test_cases_that_start_above_zero_are_indexed_from_the_first() {
    check(
        &diamond("i16", &[(5, 100), (6, 90), (7, 80), (8, 70)], 0),
        "define i16 @f(i16 %x) {
b0:
  %switch.tableidx = sub i16 %x, 5
  %0 = icmp ult i16 %switch.tableidx, 4
  %switch.idx.mult = mul nsw i16 %switch.tableidx, -10
  %switch.offset = add nsw i16 %switch.idx.mult, 100
  %1 = select i1 %0, i16 %switch.offset, i16 0
  ret i16 %1
}
",
        &[&[4], &[5], &[6], &[7], &[8], &[9]],
    );
}

/// Cases -1 to 2: the index is `x - -1`, which may wrap, so no `nsw`.
#[test]
fn test_cases_that_start_below_zero_subtract_without_nsw() {
    check(
        &diamond("i16", &[(-1, 4), (0, 6), (1, 8), (2, 10)], 0),
        "define i16 @f(i16 %x) {
b0:
  %switch.tableidx = sub i16 %x, -1
  %0 = icmp ult i16 %switch.tableidx, 4
  %switch.idx.mult = mul nsw i16 %switch.tableidx, 2
  %switch.offset = add nsw i16 %switch.idx.mult, 4
  %1 = select i1 %0, i16 %switch.offset, i16 0
  ret i16 %1
}
",
        &[&[-2], &[-1], &[0], &[1], &[2], &[3]],
    );
}

/// Three cases that give one value and a default that gives another stayed a
/// switch and a phi.
#[test]
fn test_cases_that_agree_give_their_value_behind_a_range_check() {
    check(
        &diamond("i16", &[(0, 5), (1, 5), (2, 5)], 7),
        "define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 3
  %1 = select i1 %0, i16 5, i16 7
  ret i16 %1
}
",
        &[&[0], &[1], &[2], &[3], &[-1]],
    );
}

/// Cases 0, 100, 200, 300 are too sparse for a table: the switch stays.
#[test]
fn test_a_sparse_switch_stays() {
    let text = diamond("i16", &[(0, 1), (100, 2), (200, 3), (300, 4)], 0);
    check(
        &text,
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %end
    i16 100, label %c1
    i16 200, label %c2
    i16 300, label %c3
  ]

c1:
  br label %end

c2:
  br label %end

c3:
  br label %end

d:
  br label %end

end:
  %p = phi i16 [ 2, %c1 ], [ 3, %c2 ], [ 4, %c3 ], [ 0, %d ], [ 1, %b0 ]
  ret i16 %p
}
",
        &[&[0], &[100], &[200], &[300], &[5]],
    );
}

/// With no default the index needs no range check: `x - 3` is `nsw` and the
/// result is one multiply and one add.
#[test]
fn test_an_unreachable_default_needs_no_range_check() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 3, label %a
    i16 4, label %b
    i16 5, label %c
  ]

a:
  br label %end

b:
  br label %end

c:
  br label %end

d:
  unreachable

end:
  %p = phi i16 [ 9, %a ], [ 12, %b ], [ 15, %c ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x) {
b0:
  %switch.tableidx = sub nsw i16 %x, 3
  %switch.idx.mult = mul nsw i16 %switch.tableidx, 3
  %switch.offset = add nsw i16 %switch.idx.mult, 9
  ret i16 %switch.offset
}
",
        &[&[3], &[4], &[5]],
    );
}

/// The index is `x & 3`, so the default can never run: the table covers the
/// four values, with the default's 40 as the fourth, and the range check goes.
#[test]
fn test_a_default_that_extends_the_line_is_folded_into_it() {
    check(
        "define i8 @f(i8 %y) {
b0:
  %x = and i8 %y, 3
  switch i8 %x, label %d [
    i8 0, label %a
    i8 1, label %b
    i8 2, label %c
  ]

a:
  br label %end

b:
  br label %end

c:
  br label %end

d:
  br label %end

end:
  %p = phi i8 [ 10, %a ], [ 20, %b ], [ 30, %c ], [ 40, %d ]
  ret i8 %p
}
",
        "define i8 @f(i8 %y) {
b0:
  %x = and i8 %y, 3
  %switch.idx.mult = mul nsw i8 %x, 10
  %switch.offset = add nsw i8 %switch.idx.mult, 10
  ret i8 %switch.offset
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[7]],
    );
}

/// Holes at 2, a default that calls: a mask of the cases (27) decides between
/// the one value and the default.
#[test]
fn test_holes_with_a_default_that_is_not_a_constant_are_tested_by_a_mask() {
    check(
        "define i16 @g(i16 %a) {
b0:
  ret i16 %a
}

define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %a
    i16 1, label %a
    i16 3, label %a
    i16 4, label %a
  ]

a:
  br label %end

d:
  %r = call i16 @g(i16 %x)
  br label %end

end:
  %p = phi i16 [ 7, %a ], [ %r, %d ]
  ret i16 %p
}
",
        "define i16 @g(i16 %a) {
b0:
  ret i16 %a
}

define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 5
  %1 = trunc i16 %x to i8
  %2 = lshr i8 27, %1
  %3 = trunc i8 %2 to i1
  %or.cond = select i1 %0, i1 %3, i1 false
  br i1 %or.cond, label %end, label %d

d:
  %r = call i16 @g(i16 %x)
  br label %end

end:
  %p = phi i16 [ %r, %d ], [ 7, %b0 ]
  ret i16 %p
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[5]],
    );
}

/// `p == 0` told the default from every case, so the compare was left to
/// jump-thread; it is the range check's own result, inverted.
#[test]
fn test_a_compare_of_the_result_with_the_default_reuses_the_range_check() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %a
    i16 1, label %b
    i16 2, label %c
  ]

a:
  br label %end

b:
  br label %end

c:
  br label %end

d:
  br label %end

end:
  %p = phi i16 [ 1, %a ], [ 2, %b ], [ 3, %c ], [ 0, %d ]
  %q = icmp eq i16 %p, 0
  br i1 %q, label %z, label %nz

z:
  ret i16 100

nz:
  ret i16 %p
}
",
        "define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 3
  %inverted.cmp = xor i1 %0, true
  %switch.offset = add nsw i16 %x, 1
  %1 = select i1 %0, i16 %switch.offset, i16 0
  %q = icmp eq i16 %1, 0
  %2 = select i1 %inverted.cmp, i16 100, i16 %1
  ret i16 %2
}
",
        &[&[0], &[1], &[2], &[3], &[-5]],
    );
}

/// Each case computes `x << 1` in its block, which is a constant there: the
/// results are 0, 2, 4.
#[test]
fn test_a_case_block_that_computes_from_the_condition_is_folded() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %a
    i16 1, label %b
    i16 2, label %c
  ]

a:
  %t = shl i16 %x, 1
  br label %end

b:
  %u = shl i16 %x, 1
  br label %end

c:
  %v = shl i16 %x, 1
  br label %end

d:
  br label %end

end:
  %p = phi i16 [ %t, %a ], [ %u, %b ], [ %v, %c ], [ 100, %d ]
  ret i16 %p
}
",
        "define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 3
  %switch.idx.mult = mul nsw i16 %x, 2
  %1 = select i1 %0, i16 %switch.idx.mult, i16 100
  ret i16 %1
}
",
        &[&[0], &[1], &[2], &[3], &[9]],
    );
}

/// By `y & 3` the default is dead; results 10, 20, 30, 0 are not a line, and
/// four i8 cells make a 32-bit map, read by a shift.
#[test]
fn test_values_that_are_not_a_line_are_a_bit_map() {
    check(
        "define i8 @f(i8 %y) {
b0:
  %x = and i8 %y, 3
  switch i8 %x, label %d [
    i8 0, label %a
    i8 1, label %b
    i8 2, label %c
  ]

a:
  br label %end

b:
  br label %end

c:
  br label %end

d:
  br label %end

end:
  %p = phi i8 [ 10, %a ], [ 20, %b ], [ 30, %c ], [ 0, %d ]
  ret i8 %p
}
",
        "define i8 @f(i8 %y) {
b0:
  %x = and i8 %y, 3
  %switch.cast = zext i8 %x to i32
  %switch.shiftamt = mul nuw nsw i32 %switch.cast, 8
  %switch.downshift = lshr i32 1971210, %switch.shiftamt
  %switch.masked = trunc i32 %switch.downshift to i8
  ret i8 %switch.masked
}
",
        &[&[0], &[1], &[2], &[3], &[4]],
    );
}

/// Two phis: one linear, one with a value off its line. The second is read from
/// an array global; the first stays arithmetic.
#[test]
fn test_a_phi_off_the_line_is_read_from_an_array() {
    check(
        "define i16 @f(i16 %x) {
b0:
  switch i16 %x, label %d [
    i16 0, label %a
    i16 1, label %b
    i16 2, label %c
  ]

a:
  br label %end

b:
  br label %end

c:
  br label %end

d:
  br label %end

end:
  %p = phi i16 [ 1, %a ], [ 3, %b ], [ 5, %c ], [ 0, %d ]
  %q = phi i16 [ 8, %a ], [ 8, %b ], [ 9, %c ], [ 8, %d ]
  %s = add i16 %p, %q
  ret i16 %s
}
",
        "@switch.table.0 = private constant [3 x i8] [i8 8, i8 8, i8 9]

define i16 @f(i16 %x) {
b0:
  %0 = icmp ult i16 %x, 3
  br i1 %0, label %switch.lookup, label %end

switch.lookup:
  %switch.idx.mult = mul nsw i16 %x, 2
  %switch.offset = add nsw i16 %switch.idx.mult, 1
  %1 = zext nneg i16 %x to i32
  %switch.gep = getelementptr inbounds [3 x i8], ptr @switch.table.0, i32 0, i32 %1
  %switch.load = load i8, ptr %switch.gep
  %switch.ext = zext i8 %switch.load to i16
  br label %end

end:
  %p = phi i16 [ %switch.offset, %switch.lookup ], [ 0, %b0 ]
  %q = phi i16 [ %switch.ext, %switch.lookup ], [ 8, %b0 ]
  %s = add i16 %p, %q
  ret i16 %s
}
",
        &[&[0], &[1], &[2], &[3], &[-1]],
    );
}

/// Ten cases with i32 results below 3 are read from an array of bytes.
#[test]
fn test_array_cells_are_as_narrow_as_the_values_need() {
    check(
        &diamond("i32", &[(0, 0), (1, 1), (2, 2), (3, 1), (4, 0), (5, 2), (6, 1), (7, 0), (8, 2), (9, 1)], 0),
        "@switch.table.0 = private constant [10 x i8] [i8 0, i8 1, i8 2, i8 1, i8 0, i8 2, i8 1, i8 0, i8 2, i8 1]

define i32 @f(i32 %x) {
b0:
  %0 = icmp ult i32 %x, 10
  br i1 %0, label %switch.lookup, label %end

switch.lookup:
  %switch.gep = getelementptr inbounds [10 x i8], ptr @switch.table.0, i32 0, i32 %x
  %switch.load = load i8, ptr %switch.gep
  %switch.ext = zext i8 %switch.load to i32
  br label %end

end:
  %p = phi i32 [ %switch.ext, %switch.lookup ], [ 0, %b0 ]
  ret i32 %p
}
",
        &[&[0], &[1], &[2], &[3], &[4], &[5], &[6], &[7], &[8], &[9], &[10]],
    );
}

/// `check` on a 16-bit target, where the table index is a `trunc`.
fn check16(
    text: &str,
    expected: &str,
    inputs: &[&[i128]],
) {
    let mut module = parsed(text);
    let before = results(&module, inputs);
    let outer = Outer::of(&module, Some(std::rc::Rc::new(llrm_x86_m16::Dos::default())));
    let layout = DataLayout::parse("e-p:16:16-i32:16-n8:16:32").expect("a layout");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let spec = Speculation { tti: &tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&Options::L1438, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("the declarations are placed");
    assert_eq!(printed(&module), expected);
    assert_eq!(results(&module, inputs), before);
}

/// bench/switch.c at m16 -Os grew 4 bytes: the table load, behind its range
/// check, ran before it as a `select` (the target's no-trap rule), and the
/// select lowered with extra copies. The pin keeps the load behind the check.
#[test]
fn test_the_table_load_stays_behind_its_range_check() {
    let cases: Vec<(i128, i128)> =
        (0..12).map(|at| (at, [3, 14, 15, 92, 65, 35, 89, 79, 32, 38, 46, 26][at as usize])).collect();
    let mut text = String::from("define i32 @f(i32 %x) {\nb0:\n  %m = srem i32 %x, 14\n  switch i32 %m, label %d [\n");
    for (at, (case, _)) in cases.iter().enumerate() {
        text += &format!("    i32 {case}, label %c{at}\n");
    }
    text += "  ]\n\n";
    for at in 0..cases.len() {
        text += &format!("c{at}:\n  br label %end\n\n");
    }
    text += "d:\n  br label %end\n\nend:\n  %p = phi i32 ";
    let arms: Vec<String> = cases.iter().enumerate().map(|(at, (_, v))| format!("[ {v}, %c{at} ]")).collect();
    text += &format!("{}, [ 1, %d ]\n  ret i32 %p\n}}\n", arms.join(", "));
    check16(
        &text,
        "@switch.table.0 = private constant [12 x i8] [i8 3, i8 14, i8 15, i8 92, i8 65, i8 35, i8 89, i8 79, i8 32, i8 38, i8 46, i8 26]

define i32 @f(i32 %x) {
b0:
  %m = srem i32 %x, 14
  %0 = icmp ult i32 %m, 12
  br i1 %0, label %switch.lookup, label %end

switch.lookup:
  %1 = trunc i32 %m to i16
  %switch.gep = getelementptr inbounds [12 x i8], ptr @switch.table.0, i16 0, i16 %1
  %switch.load = load i8, ptr %switch.gep
  %switch.ext = zext i8 %switch.load to i32
  br label %end

end:
  %p = phi i32 [ %switch.ext, %switch.lookup ], [ 1, %b0 ]
  ret i32 %p
}
",
        &[&[0], &[1], &[13], &[14], &[-1], &[25]],
    );
}

/// `-fno-jump-tables` reached the switch-to-lookup conversion only as the
/// "no-jump-tables" attribute clang writes, which llrm-c never did:
/// x_switch_dense at -O2 with `-fno-tree-switch-conversion -fno-jump-tables`
/// had 2 compares where the pin's has 28.
#[test]
fn test_no_jump_tables_keeps_the_switch_a_tree_of_compares() {
    let cases: Vec<(i128, i128)> =
        (0..12).map(|at| (at, [3, 14, 15, 92, 65, 35, 89, 79, 32, 38, 46, 26][at as usize])).collect();
    let text = diamond("i32", &cases, 1);
    let mut module = parsed(&text);
    let outer = Outer::of(&module, None);
    let layout = DataLayout::parse("e-p:32:32-i64:32-n8:16:32").expect("a layout");
    let tti = X86::of_cpu("i486").expect("a cpu");
    let spec = Speculation { tti: &tti, layout: &layout, outer: &outer };
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    let options = Options { no_jump_tables: true, ..Options::L1438 };
    simplified_with(&options, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("the declarations are placed");
    assert!(!printed(&module).contains("switch.table"), "{}", printed(&module));
    // With the option off the same switch is a table.
    let mut module = parsed(&text);
    let mut declared = Declared::of(&module);
    let (context, function) = module.function_mut("f").expect("@f");
    simplified_with(&Options::L1438, context, &spec, &mut declared, function);
    declared.place(&mut module).expect("the declarations are placed");
    assert!(printed(&module).contains("switch.table"), "premise: {}", printed(&module));
}
