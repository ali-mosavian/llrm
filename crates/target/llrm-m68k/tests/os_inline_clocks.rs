//! -Os counts bytes and clocks against the best compiler's. `pick` (u64_shift_select_arms: a 64-bit shift whose
//! result is cut to its low word, four calls) stayed a call at -Os: the size bound refused the inline (+72 B), the
//! bytes bar had 118 B to spare and the program ran 8606 clocks against clang's 7780. OS-INLINE-TRIAL-CLOCKS builds
//! the inline too and keeps it where the finished code is faster within the bytes ceiling.

use llrm_m68k::driver::{Level, assembly, cpu_of, object};

const PICK: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define internal i64 @pick(i32 %a, i32 %e) {
  %and = and i32 %a, 8388607
  %m32 = or i32 %and, 8388608
  %m = zext i32 %m32 to i64
  %c = icmp sgt i32 %e, 22
  %s1 = add i32 %e, -23
  %s1z = zext i32 %s1 to i64
  %l = shl i64 %m, %s1z
  %s2 = sub i32 23, %e
  %s2z = zext i32 %s2 to i64
  %r = lshr i64 %m, %s2z
  %sel = select i1 %c, i64 %l, i64 %r
  ret i64 %sel
}
define i32 @main() optsize minsize {
  %a = alloca i32
  %e = alloca i32
  store volatile i32 1081081856, ptr %a
  store volatile i32 1, ptr %e
  %a0 = load volatile i32, ptr %a
  %e0 = load volatile i32, ptr %e
  %p0 = call i64 @pick(i32 %a0, i32 %e0)
  %t0 = trunc i64 %p0 to i32
  call void @report(i32 %t0)
  store volatile i32 23, ptr %e
  %a1 = load volatile i32, ptr %a
  %e1 = load volatile i32, ptr %e
  %p1 = call i64 @pick(i32 %a1, i32 %e1)
  %t1 = trunc i64 %p1 to i32
  call void @report(i32 %t1)
  store volatile i32 24, ptr %e
  %a2 = load volatile i32, ptr %a
  %e2 = load volatile i32, ptr %e
  %p2 = call i64 @pick(i32 %a2, i32 %e2)
  %t2 = trunc i64 %p2 to i32
  call void @report(i32 %t2)
  store volatile i32 0, ptr %e
  %a3 = load volatile i32, ptr %a
  %e3 = load volatile i32, ptr %e
  %p3 = call i64 @pick(i32 %a3, i32 %e3)
  %t3 = trunc i64 %p3 to i32
  call void @report(i32 %t3)
  ret i32 0
}";

#[test]
fn the_faster_inline_is_kept_at_os_within_the_bytes_ceiling() {
    let cpu = cpu_of("68000").unwrap();
    let asm = assembly(PICK, cpu, Level::Os).expect("compiles");
    assert!(!asm.contains("pick"), "pick is still a function at -Os (8606 clk as a call, 7704 inlined):\n{asm}");
    // the same program with the call kept: the ceiling is a half more
    let plain = PICK.replace("define internal i64 @pick(i32 %a, i32 %e) {", "define internal i64 @pick(i32 %a, i32 %e) noinline {");
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
    let unit = PICK.replace("define i32 @main()", "define i32 @entry()");
    let (kept, called) = (
        object(&unit, cpu, Level::Os, "t.ll").unwrap().len(),
        object(&unit.replace("define internal i64 @pick(i32 %a, i32 %e) {", "define internal i64 @pick(i32 %a, i32 %e) noinline {"), cpu, Level::Os, "t.ll").unwrap().len(),
    );
    assert_eq!(kept, called, "a library unit grew: {kept} B against {called} B with the call kept");
}
