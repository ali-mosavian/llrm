//! -Os counts bytes and clocks against the best compiler's. `pick` (u64_shift_select_arms: a 64-bit shift whose
//! result is cut to its low word, four calls) stayed a call at -Os: the size bound refused the inline (+72 B), the
//! bytes bar had 118 B to spare and the program ran 8606 clocks against clang's 7780. OS-INLINE-TRIAL-CLOCKS builds
//! the inline too and keeps it where the finished code is faster within the bytes ceiling.

use llrm_m68k::driver::{Level, assembly, cpu_of, object};

/// Each call's arguments are read, then a volatile store sits before the call: #10057 fuses volatile loads into
/// the pushes when nothing is between, which made the call build 8 B and 16 clk cheaper and the inline (+32 % bytes
/// for about -10 % real clocks, 8590 against 7704) no longer pay: `trial::pays_tests` records that pair. The store
/// keeps the loads unfused, the shape this trial was made for.
const PICK: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define dso_local i32 @main() nounwind optsize {
  %a = alloca i32, align 2
  %e = alloca i32, align 2
  %s = alloca i32, align 2
  store volatile i32 1081081856, ptr %a, align 2
  store volatile i32 1, ptr %e, align 2
  %a0 = load volatile i32, ptr %a, align 2
  %e0 = load volatile i32, ptr %e, align 2
  store volatile i32 0, ptr %s, align 2
  %p0 = call i64 @pick(i32 %a0, i32 %e0)
  %t0 = trunc i64 %p0 to i32
  call void @report(i32 %t0)
  store volatile i32 23, ptr %e, align 2
  %a1 = load volatile i32, ptr %a, align 2
  %e1 = load volatile i32, ptr %e, align 2
  store volatile i32 0, ptr %s, align 2
  %p1 = call i64 @pick(i32 %a1, i32 %e1)
  %t1 = trunc i64 %p1 to i32
  call void @report(i32 %t1)
  store volatile i32 24, ptr %e, align 2
  %a2 = load volatile i32, ptr %a, align 2
  %e2 = load volatile i32, ptr %e, align 2
  store volatile i32 0, ptr %s, align 2
  %p2 = call i64 @pick(i32 %a2, i32 %e2)
  %t2 = trunc i64 %p2 to i32
  call void @report(i32 %t2)
  store volatile i32 0, ptr %e, align 2
  %a3 = load volatile i32, ptr %a, align 2
  %e3 = load volatile i32, ptr %e, align 2
  store volatile i32 0, ptr %s, align 2
  %p3 = call i64 @pick(i32 %a3, i32 %e3)
  %t3 = trunc i64 %p3 to i32
  call void @report(i32 %t3)
  ret i32 0
}
define internal i64 @pick(i32 %a, i32 %e) nounwind optsize {
  %pa = alloca i32, align 2
  %pe = alloca i32, align 2
  %pm = alloca i64, align 8
  store i32 %a, ptr %pa, align 2
  store i32 %e, ptr %pe, align 2
  %v = load i32, ptr %pa, align 2
  %and = and i32 %v, 8388607
  %or = or i32 %and, 8388608
  %m = zext i32 %or to i64
  store i64 %m, ptr %pm, align 8
  %ev = load i32, ptr %pe, align 2
  %c = icmp sge i32 %ev, 23
  br i1 %c, label %left, label %right
left:
  %lm = load i64, ptr %pm, align 8
  %le = load i32, ptr %pe, align 2
  %ls = sub nsw i32 %le, 23
  %lz = zext i32 %ls to i64
  %l = shl i64 %lm, %lz
  br label %join
right:
  %rm = load i64, ptr %pm, align 8
  %re = load i32, ptr %pe, align 2
  %rs = sub nsw i32 23, %re
  %rz = zext i32 %rs to i64
  %r = lshr i64 %rm, %rz
  br label %join
join:
  %sel = phi i64 [ %l, %left ], [ %r, %right ]
  ret i64 %sel
}";

#[test]
fn the_faster_inline_is_kept_at_os_within_the_bytes_ceiling() {
    let cpu = cpu_of("68000").unwrap();
    let asm = assembly(PICK, cpu, Level::Os).expect("compiles");
    assert!(!asm.contains("pick"), "pick is still a function at -Os (8606 clk as a call, 7704 inlined):\n{asm}");
    // the same program with the call kept: the ceiling is a half more
    let plain = PICK.replace("@pick(i32 %a, i32 %e) nounwind optsize {", "@pick(i32 %a, i32 %e) nounwind optsize noinline {");
    let (kept, called) = (object(PICK, cpu, Level::Os, "t.ll").unwrap().len(), object(&plain, cpu, Level::Os, "t.ll").unwrap().len());
    assert!(kept * 2 <= called * 3, "the inline is {kept} B against {called} B with the call");
}

/// stack_arrays_copy: inlining `bench` turned `bench(63) * 100` from a `muls` into a `__mulsi3` call. The routine is
/// 54 B the program links, which the trial did not count: 100 B became 174 B linked against gcc's 110.
const BENCH: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define i32 @main() optsize minsize {
  %a = call i32 @bench(i32 63)
  %m = mul nsw i32 %a, 100
  %b = call i32 @bench(i32 5)
  %s = add nsw i32 %m, %b
  call void @report(i32 %s)
  ret i32 0
}
define internal i32 @bench(i32 %n) nounwind optsize minsize {
  %a = alloca [70 x i8], align 1
  %b = alloca [70 x i8], align 1
  br label %l1
l1:
  %i = phi i32 [ 0, %0 ], [ %i1, %l1 ]
  %c = trunc i32 %i to i8
  %pa = getelementptr inbounds [70 x i8], ptr %a, i32 0, i32 %i
  store i8 %c, ptr %pa, align 1
  %i1 = add nsw i32 %i, 1
  %e1 = icmp slt i32 %i1, 70
  br i1 %e1, label %l1, label %h2
h2:
  %lim = and i32 %n, 63
  %z = icmp sgt i32 %lim, 0
  br i1 %z, label %l2, label %out
l2:
  %j = phi i32 [ 0, %h2 ], [ %j1, %l2 ]
  %qa = getelementptr inbounds [70 x i8], ptr %a, i32 0, i32 %j
  %v = load i8, ptr %qa, align 1
  %qb = getelementptr inbounds [70 x i8], ptr %b, i32 0, i32 %j
  store i8 %v, ptr %qb, align 1
  %j1 = add nsw i32 %j, 1
  %e2 = icmp slt i32 %j1, %lim
  br i1 %e2, label %l2, label %out
out:
  %p3 = getelementptr inbounds [70 x i8], ptr %b, i32 0, i32 3
  %r = load i8, ptr %p3, align 1
  %x = sext i8 %r to i32
  ret i32 %x
}";

#[test]
fn an_inline_that_links_the_multiply_routine_pays_for_its_bytes() {
    let cpu = cpu_of("68000").unwrap();
    let asm = assembly(BENCH, cpu, Level::Os).expect("compiles");
    assert!(!asm.contains("__mulsi3"), "the inline brought in __mulsi3 (54 B linked, not priced):\n{asm}");
}

/// The runtime library is built at -Os unit by unit: with no `main` the trial weighs each function as run once, and
/// its clocks were phantom there. libm's `mathf.o` grew 4234 -> 5006 B (-Os) and every float program that links it
/// ran the same clocks (`floats` 68000 -Os 3038 -> 3230 B, 30511034 clk before and after).
#[test]
fn a_unit_without_main_is_not_trialled_for_clocks() {
    let cpu = cpu_of("68000").unwrap();
    let unit = PICK.replace("define dso_local i32 @main()", "define dso_local i32 @entry()");
    let (kept, called) = (
        object(&unit, cpu, Level::Os, "t.ll").unwrap().len(),
        object(&unit.replace("@pick(i32 %a, i32 %e) nounwind optsize {", "@pick(i32 %a, i32 %e) nounwind optsize noinline {"), cpu, Level::Os, "t.ll").unwrap().len(),
    );
    assert_eq!(kept, called, "a library unit grew: {kept} B against {called} B with the call kept");
}
