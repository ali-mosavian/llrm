//! `WidenIV` through `indvars`: a counter read through `sext` or `zext` is a
//! counter of the extension's width.

use crate::indvars::IndVars;
use crate::testing::{managed, parsed, printed, results};

/// `for (i = 0; i < n; i++) buf[ext i] = 1` with `i` of 16 bits and the
/// extension to 32.
fn clearing(
    extension: &str,
    predicate: &str,
    start: &str,
    step: &str,
) -> String {
    format!(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 %n) {{
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ {start}, %b0 ], [ %next, %b2 ]
  %go = icmp {predicate} i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %w = {extension} i16 %i to i32
  %v = add i32 %w, 8
  %p = getelementptr inbounds i8, ptr %buf, i32 %v
  store i8 1, ptr %p
  %next = add {step} i16 %i, 1
  br label %b1

b3:
  ret i32 0
}}
"
    )
}

/// @f of `text` through `indvars` (with the exit values), printed, and
/// computing what it did on `inputs`.
fn widened(text: &str) -> String {
    let mut module = parsed(text);
    let inputs: &[&[i128]] = &[&[0], &[1], &[7], &[40]];
    let before = results(&module, inputs);
    managed(&mut module, IndVars { lftr: true, size: false });
    let after = printed(&module);
    assert_eq!(results(&module, inputs), before, "{after}");
    after
}

#[test]
fn test_a_counter_read_through_zext_is_a_counter_of_that_width() {
    let after = widened(&clearing("zext", "ult", "0", "nsw"));
    eprintln!("{after}");
    assert!(after.contains("phi i32"), "{after}");
    assert!(!after.contains("phi i16"), "the narrow counter is gone: {after}");
    assert!(!after.contains("zext i16 %i to i32"), "{after}");
}

#[test]
fn test_a_signed_counter_read_through_sext_is_a_counter_of_that_width() {
    let after = widened(&clearing("sext", "slt", "0", "nsw"));
    assert!(after.contains("phi i32"), "{after}");
    // LFTR then tests the wide counter against its exit count with `ne`.
    assert!(after.contains("icmp ne i32"), "{after}");
    assert!(!after.contains("sext i16 %i to i32"), "{after}");
}

#[test]
fn test_a_signed_counter_from_a_negative_start_is_widened_from_the_negative_number() {
    let after = widened(&clearing("sext", "slt", "-5", "nsw"));
    assert!(after.contains("[ -5, %b0 ]"), "{after}");
}

#[test]
fn test_an_unsigned_counter_read_through_sext_is_not_widened() {
    let after = widened(&clearing("sext", "ult", "0", ""));
    assert!(!after.contains("phi i32"), "{after}");
}

/// The 16-bit target's X86 model and `n8:16:32` call `i32` legal: a counter is
/// not widened past the pointer's 16 bits there (D18), where its `i32` add
/// costs two.
#[test]
fn test_a_counter_is_not_widened_past_the_pointer_width_on_the_16_bit_target() {
    let text = clearing("zext", "ult", "0", "nsw").replace("e-p:32:32-n8:16:32", "e-p:16:16-n8:16:32");
    let mut module = parsed(&text);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(IndVars { lftr: true, size: false });
    manager.run_module(&mut module, std::rc::Rc::new(llrm_x86_m16::Dos::default())).unwrap();
    let after = printed(&module);
    assert!(!after.contains("phi i32"), "{after}");
}

/// A 32-bit pointer with a legal `i64` (wasm32's `n32:64`) widens an `i32`
/// counter that a `zext` takes to `i64`: the pin does, and D18's pointer-width
/// bound held it back. shift_int64_variable -O1 and -Os kept an `i32` counter
/// and a second `i64` one for the shift amount: +12 B.
#[test]
fn test_a_counter_zero_extended_to_i64_is_widened_with_a_32_bit_pointer() {
    let text = "target datalayout = \"e-p:32:32-i64:64-n32:64\"

define i64 @f(i64 %x) {
b0:
  br label %b1

b1:
  %i = phi i32 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i64 [ 0, %b0 ], [ %sum, %b1 ]
  %w = zext nneg i32 %i to i64
  %s = shl i64 %x, %w
  %sum = add i64 %acc, %s
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 40
  br i1 %done, label %b2, label %b1

b2:
  ret i64 %sum
}
";
    let mut module = parsed(text);
    managed(&mut module, IndVars { lftr: true, size: false });
    let after = printed(&module);
    assert!(after.contains("phi i64 [ 0, %b0 ], [ %") && !after.contains("phi i32"), "{after}");
}

/// A user of the counter with no value (a `store` of it) is a use to truncate
/// for. `widen_iv_use` asked every user for its value first: `bench/bintree`
/// at -O2 -m32 panicked 'an instruction with a value' (#2531).
#[test]
fn test_a_counter_a_store_reads_is_widened_and_the_store_reads_a_truncation() {
    let text = "target datalayout = \"e-p:32:32-n8:16:32\"

@g = global i16 0

define i32 @f(i16 %n) {
b0:
  %buf = alloca [64 x i8]
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  store i16 %i, ptr @g
  %w = sext i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add nsw i16 %i, 1
  br label %b1

b3:
  ret i32 0
}
";
    let after = widened(text);
    assert!(after.contains("phi i32"), "{after}");
    assert!(after.contains("= trunc nuw nsw i32"), "{after}");
}

/// histo-O2 `pipeline.body`: the `int` index `%i` and the `short` counter
/// `%j` the loop tests run side by side; `%i` is `sext` of `%j`'s next value.
/// The narrow counter stayed, with the `sext` and an `slt i16` test, where
/// `opt` has one `i32` counter and `icmp ne i32 %i.next, 4096`.
#[test]
fn test_a_narrow_counter_beside_its_own_sign_extension_is_not_kept() {
    let after = widened(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f() {
b0:
  %buf = alloca [4096 x i8]
  br label %b1

b1:
  %i = phi i32 [ %w, %b1 ], [ 0, %b0 ]
  %j = phi i16 [ %next, %b1 ], [ 0, %b0 ]
  %p = getelementptr inbounds i8, ptr %buf, i32 %i
  store i8 1, ptr %p
  %next = add i16 %j, 1
  %w = sext i16 %next to i32
  %go = icmp slt i16 %next, 4096
  br i1 %go, label %b1, label %b2

b2:
  ret i32 0
}
",
    );
    assert!(!after.contains("phi i16"), "{after}");
    assert!(!after.contains("sext i16"), "{after}");
}

/// induction32: the narrow increment feeds the phi and the exit test, so the
/// wide add was cloned for each; LFTR then found no counter and the exit test
/// stayed `icmp ult i32` beside two identical adds.
#[test]
fn test_one_wide_increment_serves_the_phi_and_the_exit_test() {
    let after = widened(
        "target datalayout = \"e-p:32:32-n8:16:32\"

define i32 @f(i16 zeroext %a) {
b0:
  %buf = alloca [64 x i8]
  %s = add i16 %a, 1
  %go = icmp ult i16 %s, 4
  br i1 %go, label %pre, label %b3

pre:
  br label %b1

b1:
  %i = phi i16 [ %s, %pre ], [ %next, %b1 ]
  %w = zext nneg i16 %i to i32
  %p = getelementptr inbounds i8, ptr %buf, i32 %w
  store i8 1, ptr %p
  %next = add i16 %i, 1
  %c = icmp ult i16 %next, 4
  br i1 %c, label %b1, label %out

out:
  br label %b3

b3:
  ret i32 0
}
",
    );
    assert_eq!(after.matches("add nuw nsw i32").count(), 1, "{after}");
    assert!(after.contains("icmp ne i16"), "the exit test is replaced: {after}");
}

/// quicksort's `sort` at -O2 on m64, as it reaches `indvars`: REC5-1's ranges
/// make `lo` non-negative, so the counters' address uses are `zext nneg`; the
/// inner counter starts at the outer phi and is compared signed. The compare
/// stayed `icmp slt i32 (trunc nuw i64 %iv.next), %hi`, one `lea` more per
/// iteration (141968 instructions per call against 129992).
#[test]
fn test_a_counter_with_a_zext_nneg_use_is_widened_with_its_signed_compare() {
    let text = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"

define void @sort(ptr %0, i32 range(i32 0, -2147483648) %1, i32 range(i32 -1, 2147483647) %2) {
  %4 = icmp slt i32 %1, %2
  br i1 %4, label %5, label %37

5:
  %6 = zext nneg i32 %2 to i64
  %7 = getelementptr inbounds nuw [4 x i8], ptr %0, i64 %6
  br label %8

8:
  %9 = phi i32 [ %1, %5 ], [ %34, %27 ]
  %10 = load i32, ptr %7, align 4
  br label %11

11:
  %12 = phi i32 [ %9, %8 ], [ %25, %23 ]
  %13 = phi i32 [ %9, %8 ], [ %24, %23 ]
  %14 = zext nneg i32 %12 to i64
  %15 = getelementptr inbounds nuw [4 x i8], ptr %0, i64 %14
  %16 = load i32, ptr %15, align 4
  %17 = icmp slt i32 %16, %10
  br i1 %17, label %18, label %23

18:
  %19 = zext nneg i32 %13 to i64
  %20 = getelementptr inbounds nuw [4 x i8], ptr %0, i64 %19
  %21 = load i32, ptr %20, align 4
  store i32 %16, ptr %20, align 4
  store i32 %21, ptr %15, align 4
  %22 = add nuw nsw i32 %13, 1
  br label %23

23:
  %24 = phi i32 [ %22, %18 ], [ %13, %11 ]
  %25 = add nuw nsw i32 %12, 1
  %26 = icmp slt i32 %25, %2
  br i1 %26, label %11, label %27

27:
  %28 = phi i32 [ %24, %23 ]
  %29 = zext nneg i32 %28 to i64
  %30 = getelementptr inbounds nuw [4 x i8], ptr %0, i64 %29
  %31 = load i32, ptr %30, align 4
  %32 = load i32, ptr %7, align 4
  store i32 %32, ptr %30, align 4
  store i32 %31, ptr %7, align 4
  %33 = add nsw i32 %28, -1
  tail call void @sort(ptr %0, i32 %9, i32 %33)
  %34 = add nuw nsw i32 %28, 1
  %35 = icmp slt i32 %34, %2
  br i1 %35, label %8, label %36

36:
  br label %37

37:
  ret void
}
";
    let mut module = parsed(text);
    managed(&mut module, IndVars { lftr: true, size: false });
    let after = printed(&module);
    assert!(!after.contains("trunc"), "the compare stayed on the narrow value: {after}");
}

/// A prime's marking loop in a sieve (`bench/sieve`, bench_sieve(1024) inlined
/// into `main`): the inner counter starts at `i * i`, and the scan loop
/// reaches it only when `i` is unmarked and below 32.
const MARKING_AFTER_SQUARE: &str = "target datalayout = \"e-p:64:64-i64:64-n8:16:32:64\"

declare void @llvm.memset.p0.i64(ptr nocapture writeonly, i8, i64, i1 immarg)

define i64 @f() {
b0:
  %buf = alloca [1024 x i8], align 16
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %buf, i8 0, i64 1024, i1 false)
  br label %b1

b1:
  %i = phi i64 [ 2, %b0 ], [ %i.next, %b6 ]
  %sum = phi i64 [ 0, %b0 ], [ %sum.next, %b6 ]
  %p = getelementptr inbounds nuw i8, ptr %buf, i64 %i
  %c = load i8, ptr %p, align 1
  %free = icmp eq i8 %c, 0
  br i1 %free, label %b2, label %b6

b2:
  %add = add i64 %sum, %i
  %small = icmp samesign ult i64 %i, 32
  br i1 %small, label %b3, label %b6

b3:
  %sq = mul nuw nsw i64 %i, %i
  %start = trunc nuw nsw i64 %sq to i16
  %step = trunc nuw nsw i64 %i to i16
  br label %b4

b4:
  %m = phi i16 [ %start, %b3 ], [ %m.next, %b4 ]
  %wide = zext nneg i16 %m to i64
  %q = getelementptr inbounds nuw i8, ptr %buf, i64 %wide
  store i8 1, ptr %q, align 1
  %m.next = add nuw nsw i16 %m, %step
  %again = icmp samesign ult i16 %m.next, 1024
  br i1 %again, label %b4, label %b5

b5:
  br label %b6

b6:
  %sum.next = phi i64 [ %sum, %b1 ], [ %add, %b2 ], [ %add, %b5 ]
  %i.next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %i.next, 1024
  br i1 %done, label %b7, label %b1

b7:
  ret i64 %sum.next
}
";

fn marking(cold_outer_chrec: bool) -> String {
    let mut module = parsed(MARKING_AFTER_SQUARE);
    let machine = crate::testing::Tuned { cold_outer_chrec, ..Default::default() };
    crate::testing::managed_on(&mut module, IndVars { lftr: true, size: false }, machine)
}

/// sieve x86-64 O2/O3 ran 15223 instructions against gcc's 14703: widening
/// the marking counter expanded `i * i` as two phis of the scan loop, which
/// stepped 1022 times for 11 entries.
#[test]
fn test_a_counter_starting_at_a_square_entered_on_some_trips_stays_narrow() {
    let after = marking(true);
    assert!(after.contains("phi i16 [ %start, %b3 ]"), "the counter stays 16 bits: {after}");
    assert!(!after.contains("[ 4, %b0 ]"), "no phi of the scan loop for the square: {after}");
}

/// The pin widens it: the default target keeps the pin's IR.
#[test]
fn test_a_target_that_does_not_weigh_entries_widens_a_counter_starting_at_a_square() {
    let after = marking(false);
    assert!(after.contains("[ 4, %b0 ]"), "the square is a phi of the scan loop: {after}");
}
