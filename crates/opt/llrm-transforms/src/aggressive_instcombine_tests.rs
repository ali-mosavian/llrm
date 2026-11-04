//! Each body is MIR text run by llrm-mir's interpreter before and after, so a
//! wrong merge shows as a wrong answer; what is asserted of the text is the
//! loads left.

use llrm_analysis::testing::DOS;

use super::AggressiveInstCombine;
use crate::testing::{managed, parsed, results};

fn merged(text: &str) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    // The pin leaves the old chain to dead-code elimination.
    let after = managed(&mut module, crate::dead::Dead);
    let inputs: &[&[i128]] = &[&[0], &[1], &[4660], &[8], &[18], &[2], &[16], &[41], &[-1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

const BYTES: &str = "@g = global [4 x i8] zeroinitializer
define i32 @f() {
b0:
  %p1 = getelementptr i8, ptr @g, i32 1
  %p2 = getelementptr i8, ptr @g, i32 2
  %p3 = getelementptr i8, ptr @g, i32 3
  %b0 = load i8, ptr @g
  %b1 = load i8, ptr %p1
  %b2 = load i8, ptr %p2
  %b3 = load i8, ptr %p3
  %z0 = zext i8 %b0 to i32
  %z1 = zext i8 %b1 to i32
  %z2 = zext i8 %b2 to i32
  %z3 = zext i8 %b3 to i32
  %s1 = shl i32 %z1, 8
  %s2 = shl i32 %z2, 16
  %s3 = shl i32 %z3, 24
  %o1 = or i32 %z0, %s1
  %o2 = or i32 %o1, %s2
  %o3 = or i32 %o2, %s3
  ret i32 %o3
}
";

/// Four byte loads or'd with shifts stayed four `movzx`; the pin makes them
/// one `load i32`.
#[test]
fn four_adjacent_byte_loads_assembled_little_endian_are_one_dword_load() {
    let after = merged(BYTES);
    assert_eq!(after.matches("load ").count(), 1, "{after}");
    assert!(after.contains("load i32"), "{after}");
}

/// A store between the loads may change a byte: no merge.
#[test]
fn a_store_between_the_loads_stops_the_merge() {
    let text = BYTES.replace("  %b1 = load", "  store i8 1, ptr @g\n  %b1 = load");
    let after = merged(&text);
    assert_eq!(after.matches("load ").count(), 4, "{after}");
}

/// Bytes in the wrong order are not a little-endian dword.
#[test]
fn bytes_out_of_order_are_not_merged() {
    let text = BYTES.replace("shl i32 %z1, 8", "shl i32 %z1, 16").replace("shl i32 %z2, 16", "shl i32 %z2, 8");
    let after = merged(&text);
    assert_eq!(after.matches("load ").count(), 4, "{after}");
}

/// The high byte is loaded first and the low byte's address is made after it:
/// the wide load went before the high byte's load with the low byte's address,
/// which is not yet defined there ("does not dominate", the QB runtime's
/// `B$BLOD` at -Os m16: isel died with "value with no register").
#[test]
fn a_wide_load_is_not_placed_before_the_definition_of_its_address() {
    let text = "@g = global [4 x i8] zeroinitializer
define i16 @f() {
b0:
  %p1 = getelementptr i8, ptr @g, i16 1
  %hi = load i8, ptr %p1
  %p0 = getelementptr i8, ptr @g, i16 0
  %lo = load i8, ptr %p0
  %zh = zext i8 %hi to i16
  %zl = zext i8 %lo to i16
  %sh = shl i16 %zh, 8
  %r = or i16 %zl, %sh
  ret i16 %r
}
";
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    let after = managed(&mut module, AggressiveInstCombine);
    assert!(after.contains("load i16"), "{after}");
    let problems = llrm_mir::verify::verify(&module);
    assert!(problems.is_empty(), "{problems:?}\n{after}");
}

const HALVES: &str = "@g = global [4 x i8] zeroinitializer
define i16 @f(i16 %x) {
b0:
  %lo = trunc i16 %x to i8
  %h = lshr i16 %x, 8
  %hi = trunc i16 %h to i8
  store i8 %lo, ptr @g
  %p = getelementptr i8, ptr @g, i16 1
  store i8 %hi, ptr %p
  %r = load i16, ptr @g
  ret i16 %r
}
";

/// Two byte stores of the halves of one value stayed two `mov`s; the pin
/// makes them one `store i16`.
#[test]
fn the_two_bytes_of_a_value_stored_side_by_side_are_one_word_store() {
    let after = merged(HALVES);
    assert_eq!(after.matches("store ").count(), 1, "{after}");
    assert!(after.contains("store i16"), "{after}");
}

/// A load of the object between the stores may see the first alone: no merge.
#[test]
fn a_read_between_the_stores_stops_the_merge() {
    let text = HALVES.replace("  %p = getelementptr", "  %m = load i8, ptr @g\n  %p = getelementptr");
    let after = merged(&text);
    assert_eq!(after.matches("store ").count(), 2, "{after}");
}

/// Bytes two apart leave a gap the wider store would write.
#[test]
fn stores_with_a_gap_between_them_are_not_merged() {
    let text = HALVES.replace("i16 1\n  store", "i16 2\n  store");
    let after = merged(&text);
    assert_eq!(after.matches("store ").count(), 2, "{after}");
}

const POPCOUNT: &str = "define i32 @f(i32 %x) {
b0:
  %a = lshr i32 %x, 1
  %b = and i32 %a, 1431655765
  %c = sub i32 %x, %b
  %d = and i32 %c, 858993459
  %e = lshr i32 %c, 2
  %f = and i32 %e, 858993459
  %g = add i32 %d, %f
  %h = lshr i32 %g, 4
  %i = add i32 %g, %h
  %j = and i32 %i, 252645135
  %k = mul i32 %j, 16843009
  %l = lshr i32 %k, 24
  ret i32 %l
}
";

/// The parallel bit count (Seander Anderson's) stayed twelve instructions and
/// a multiply; the pin makes it `llvm.ctpop`.
#[test]
fn the_parallel_bit_count_is_a_ctpop() {
    let after = merged(POPCOUNT);
    assert!(after.contains("llvm.ctpop.i32"), "{after}");
    assert!(!after.contains("mul "), "{after}");
}

/// A mask that is not 0x55... is not the idiom.
#[test]
fn a_bit_count_with_another_mask_is_left_alone() {
    let after = merged(&POPCOUNT.replace("1431655765", "1431655764"));
    assert!(!after.contains("llvm.ctpop"), "{after}");
}

const CTTZ_TABLE: &str = "@t = constant [32 x i8] [i8 0, i8 1, i8 28, i8 2, i8 29, i8 14, i8 24, i8 3, i8 30, i8 22, i8 20, i8 15, i8 25, i8 17, i8 4, i8 8, i8 31, i8 27, i8 13, i8 23, i8 21, i8 19, i8 16, i8 7, i8 26, i8 12, i8 18, i8 6, i8 11, i8 5, i8 10, i8 9]
define i8 @f(i32 %x) {
b0:
  %neg = sub i32 0, %x
  %low = and i32 %neg, %x
  %mul = mul i32 %low, 125613361
  %shr = lshr i32 %mul, 27
  %idx = trunc i32 %shr to i16
  %p = getelementptr inbounds [32 x i8], ptr @t, i16 0, i16 %idx
  %v = load i8, ptr %p
  ret i8 %v
}
";

/// A de Bruijn table of trailing-zero counts was a negate, an and, a multiply,
/// a shift and a load (`ctz` in every bit-scan loop); the pin's
/// `tryToRecognizeTableBasedCttz` makes it `cttz`.
#[test]
fn a_de_bruijn_table_lookup_is_a_cttz() {
    let after = merged(CTTZ_TABLE);
    assert!(after.contains("llvm.cttz.i32"), "{after}");
    assert!(!after.contains("load i8"), "{after}");
}

const LOG2_TABLE: &str = "@t = constant [32 x i8] [i8 0, i8 9, i8 1, i8 10, i8 13, i8 21, i8 2, i8 29, i8 11, i8 14, i8 16, i8 18, i8 22, i8 25, i8 3, i8 30, i8 8, i8 12, i8 20, i8 28, i8 15, i8 17, i8 24, i8 7, i8 19, i8 27, i8 23, i8 6, i8 26, i8 5, i8 4, i8 31]
define i8 @f(i32 %v) {
b0:
  %a1 = lshr i32 %v, 1
  %o1 = or i32 %a1, %v
  %a2 = lshr i32 %o1, 2
  %o2 = or i32 %a2, %o1
  %a3 = lshr i32 %o2, 4
  %o3 = or i32 %a3, %o2
  %a4 = lshr i32 %o3, 8
  %o4 = or i32 %a4, %o3
  %a5 = lshr i32 %o4, 16
  %o5 = or i32 %a5, %o4
  %mul = mul i32 %o5, 130329821
  %shr = lshr i32 %mul, 27
  %idx = trunc i32 %shr to i16
  %p = getelementptr inbounds [32 x i8], ptr @t, i16 0, i16 %idx
  %r = load i8, ptr %p
  ret i8 %r
}
";

fn merged_on(
    text: &str,
    machine: crate::testing::Tuned,
) -> String {
    let before = parsed(&format!("{DOS}{text}"));
    let mut module = before.clone();
    crate::testing::managed_on(&mut module, AggressiveInstCombine, machine);
    let after = managed(&mut module, crate::dead::Dead);
    let inputs: &[&[i128]] = &[&[0], &[1], &[4660], &[-1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
    after
}

/// A de Bruijn log2 table behind an or cascade was five shifts, five ors, a
/// multiply and a load; where `ctlz` is one instruction the pin's
/// `tryToRecognizeTableBasedLog2` makes it `31 - ctlz`, and where it is not
/// (x86 without lzcnt: 5) it leaves the table.
#[test]
fn a_de_bruijn_log2_table_is_a_ctlz_where_ctlz_is_cheap() {
    let cheap = merged_on(LOG2_TABLE, crate::testing::Tuned { cheap_ctlz: true, ..Default::default() });
    assert!(cheap.contains("llvm.ctlz.i32"), "{cheap}");
    assert!(!cheap.contains("load i8"), "{cheap}");
    let dear = merged(LOG2_TABLE);
    assert!(dear.contains("load i8"), "no cheap ctlz: the table stays\n{dear}");
}

const MASK_TABLE: &str = "@t = constant [6 x i32] [i32 0, i32 1, i32 3, i32 7, i32 15, i32 31]
define i32 @f(i32 %x, i16 %i) {
b0:
  %p = getelementptr inbounds [6 x i32], ptr @t, i16 0, i16 %i
  %m = load i32, ptr %p
  %r = and i32 %x, %m
  ret i32 %r
}
";

/// `x & tbl[i]` through a table of low-bits masks was a load; the pin's
/// `tryToRecognizeTableBasedLowBitsMask` makes it `x & ((1 << i) - 1)`.
#[test]
fn a_table_of_low_bit_masks_is_a_shift_and_a_decrement() {
    let before = parsed(&format!("{DOS}{MASK_TABLE}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(after.contains("shl i32 1,") && after.contains("sub i32"), "{after}");
    assert!(!after.contains("load i32"), "{after}");
    let inputs: &[&[i128]] = &[&[-1, 0], &[-1, 3], &[4660, 5], &[255, 1]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
}

const GUARDED_FSHL: &str = "define i32 @f(i32 %a, i32 %b, i32 %n) {
g:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %p, label %f
f:
  %sub = sub i32 32, %n
  %shr = lshr i32 %b, %sub
  %shl = shl i32 %a, %n
  %or = or i32 %shr, %shl
  br label %p
p:
  %r = phi i32 [ %or, %f ], [ %a, %g ]
  ret i32 %r
}
";

/// A funnel shift behind a branch around a zero amount (the C idiom that
/// avoids shifting by the width) was a branch, a phi and four operations; the
/// pin's `foldGuardedFunnelShift` makes it `fshl`.
#[test]
fn a_funnel_shift_guarded_against_a_zero_amount_is_an_fshl() {
    let before = parsed(&format!("{DOS}{GUARDED_FSHL}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(after.contains("llvm.fshl.i32"), "{after}");
    let inputs: &[&[i128]] = &[&[1, 2, 0], &[1, 2, 4], &[305419896, 2596069104, 8], &[-1, 5, 31]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
}

const SHIFTED_OR_IS_ZERO: &str = "define i1 @f(i32 %a, i32 %b) {
b0:
  %s = shl nuw i32 %a, 3
  %o = or i32 %s, %b
  %c = icmp eq i32 %o, 0
  ret i1 %c
}
";

/// A no-wrap shift inside an or chain compared with zero changes nothing the
/// compare sees; the pin's `foldICmpOrChain` drops it, so the shift is not
/// computed.
#[test]
fn a_no_wrap_shift_in_an_or_chain_compared_with_zero_is_dropped() {
    let before = parsed(&format!("{DOS}{SHIFTED_OR_IS_ZERO}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(!after.contains("shl"), "{after}");
    let inputs: &[&[i128]] = &[&[0, 0], &[1, 0], &[0, 1], &[5, 7]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
}

const MUL_HIGH_LADDER: &str = "define i32 @f(i32 %x, i32 %y) {
b0:
  %xl = and i32 %x, 65535
  %xh = lshr i32 %x, 16
  %yl = and i32 %y, 65535
  %yh = lshr i32 %y, 16
  %xlyl = mul i32 %xl, %yl
  %xhyl = mul i32 %xh, %yl
  %xlyh = mul i32 %xl, %yh
  %xhyh = mul i32 %xh, %yh
  %t = lshr i32 %xlyl, 16
  %c2 = add i32 %t, %xhyl
  %c2l = and i32 %c2, 65535
  %c3 = add i32 %c2l, %xlyh
  %c2h = lshr i32 %c2, 16
  %c3h = lshr i32 %c3, 16
  %a1 = add i32 %xhyh, %c2h
  %r = add i32 %a1, %c3h
  ret i32 %r
}
";

/// The high half of a 32-bit multiply built from 16-bit parts (the ladder of
/// carries) was thirteen operations; the pin's `foldMulHigh` makes it a
/// 64-bit multiply, a shift and a truncate.
#[test]
fn the_high_half_of_a_multiply_in_parts_is_a_widening_multiply() {
    let before = parsed(&format!("{DOS}{MUL_HIGH_LADDER}"));
    let mut module = before.clone();
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(after.contains("mul nuw i64"), "{after}");
    assert!(!after.contains("and i32"), "{after}");
    let inputs: &[&[i128]] = &[&[0, 0], &[65535, 65535], &[4660, 65536], &[-1, -1], &[305419896, 2596069104]];
    assert_eq!(results(&module, inputs), results(&before, inputs), "{after}");
}

const SQRT_OF_SQUARE: &str = "declare double @sqrt(double)
define i32 @f(i32 %x) {
b0:
  %d = sitofp i32 %x to double
  %s = fmul double %d, %d
  %r = call double @sqrt(double %s)
  %i = fptosi double %r to i32
  ret i32 %i
}
";

/// `sqrt(x * x)` stayed a libcall that may set `errno`; where the target has a
/// sqrt instruction and the argument cannot be negative, the pin's `foldSqrt`
/// makes it `llvm.sqrt`.
#[test]
fn a_sqrt_of_a_square_is_the_intrinsic_where_the_target_has_a_sqrt() {
    // Text only: the interpreter has no external `sqrt`.
    let run = |machine: crate::testing::Tuned| {
        let mut module = parsed(&format!("{DOS}{SQRT_OF_SQUARE}"));
        crate::testing::managed_on(&mut module, AggressiveInstCombine, machine);
        managed(&mut module, crate::dead::Dead)
    };
    let fast = run(crate::testing::Tuned { fast_sqrt: true, ..Default::default() });
    assert!(fast.contains("llvm.sqrt.f64"), "{fast}");
    assert!(!fast.contains("call double @sqrt"), "{fast}");
    let slow = run(Default::default());
    assert!(slow.contains("call double @sqrt"), "no sqrt instruction: the call stays\n{slow}");
}

const MEMCHR: &str = "@s = constant [3 x i8] c\"abc\"
declare ptr @memchr(ptr, i16, i16)
define i16 @f(i16 %c) {
b0:
  %p = call ptr @memchr(ptr @s, i16 %c, i16 3)
  %base = ptrtoint ptr @s to i16
  %at = ptrtoint ptr %p to i16
  %off = sub i16 %at, %base
  %none = icmp eq ptr %p, null
  %r = select i1 %none, i16 -1, i16 %off
  ret i16 %r
}
";

/// `memchr` of a three-byte constant string was a library call; the pin's
/// `foldMemChr` makes it a switch on the byte to the index of its first
/// occurrence, null otherwise. The folded body is run for every byte it can
/// find and for two it cannot.
#[test]
fn a_memchr_of_a_short_constant_string_is_a_switch_on_the_byte() {
    let mut module = parsed(&format!("{DOS}{MEMCHR}"));
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(!after.contains("call ptr @memchr"), "{after}");
    assert!(after.contains("switch i8"), "{after}");
    for (c, want) in [(97, 0), (98, 1), (99, 2), (100, 0xffff), (0, 0xffff)] {
        let got = results(&module, &[&[c]]);
        let want = llrm_mir::interpret::Val::Int { bits: want, width: 16 };
        assert_eq!(got, vec![want], "memchr(\"abc\", {c})\n{after}");
    }
}

const ANY_BITS: &str = "define i16 @f(i16 %x) {
b0:
  %a = lshr i16 %x, 3
  %b = lshr i16 %x, 5
  %o1 = or i16 %x, %a
  %o2 = or i16 %o1, %b
  %r = and i16 %o2, 1
  ret i16 %r
}
";

const ALL_BITS: &str = "define i16 @f(i16 %x) {
b0:
  %a = lshr i16 %x, 1
  %b = and i16 %a, 1
  %c = lshr i16 %x, 4
  %r = and i16 %b, %c
  ret i16 %r
}
";

/// Four shifts, three ors and an `and` testing three bits of `x` for any set,
/// and a shift-and chain testing two for all set, were many instructions; the
/// pin's `foldAnyOrAllBitsSet` makes each one masked compare.
#[test]
fn a_chain_of_shifted_bits_is_one_masked_compare() {
    for (text, compare) in [(ANY_BITS, "icmp ne i16"), (ALL_BITS, "icmp eq i16")] {
        let after = merged(text);
        assert!(after.contains(compare), "{after}");
        assert!(!after.contains("lshr"), "{after}");

const STRCMP: &str = "@buf = global [3 x i8] c\"xb\\00\"
@s = constant [3 x i8] c\"ab\\00\"
declare i16 @strcmp(ptr, ptr)
define i16 @f(i16 %c) {
b0:
  %byte = trunc i16 %c to i8
  store i8 %byte, ptr @buf
  %a = ptrtoint ptr @buf to i16
  %p = inttoptr i16 %a to ptr
  %r = call i16 @strcmp(ptr %p, ptr @s)
  %same = icmp eq i16 %r, 0
  %out = zext i1 %same to i16
  ret i16 %out
}
";

/// `strcmp(p, \"ab\") == 0` was a library call; the pin's `StrNCmpInliner`
/// makes it two byte differences, the second read only if the first is zero.
/// The folded body runs for the byte that matches and for two that do not.
#[test]
fn a_strcmp_with_a_short_constant_string_tested_for_equality_is_a_byte_chain() {
    let mut module = parsed(&format!("{DOS}{STRCMP}"));
    managed(&mut module, AggressiveInstCombine);
    let after = managed(&mut module, crate::dead::Dead);
    assert!(!after.contains("call i16 @strcmp"), "{after}");
    for (c, want) in [(97, 1), (98, 0), (0, 0)] {
        let got = results(&module, &[&[c]]);
        let want = llrm_mir::interpret::Val::Int { bits: want, width: 16 };
        assert_eq!(got, vec![want], "strcmp(\"{c}b\", \"ab\") == 0\n{after}");
    }
}
