//! 6502-UNROLL-O1: with `-funroll-loops` (the rows' default at -O1) a full unroll is made only where
//! it does not grow the code, and a loop the plain walk refuses is unrolled on trial when the
//! function then costs no more bytes and no more clocks. The bare flag grew byval_struct +178 B,
//! frameless +27 B, loop_fusion +10 B at -O1, which is strict on bytes (2026-11-10).

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn o1(ir: &str) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let request = Request { passes: vec!["-funroll-loops".into()], ..Request::new(Level::O1) };
    assembly(&ir, &request).unwrap_or_else(|e| panic!("{e}"))
}

/// Ten trips of a 32-bit store: unrolled it is 41 `sta` against the loop's 9 (169 lines against 73).
#[test]
fn an_o1_unroll_does_not_grow_a_store_loop() {
    let text = o1("define void @fill(ptr %a) {\nentry:\n  br label %l\nl:\n  %i = phi i16 [0, %entry], [%i1, %l]\n  \
         %p = getelementptr i32, ptr %a, i16 %i\n  %v = zext i16 %i to i32\n  store i32 %v, ptr %p, align 1\n  \
         %i1 = add nuw i16 %i, 1\n  %c = icmp ne i16 %i1, 10\n  br i1 %c, label %l, label %x\nx:\n  ret void\n}\n");
    let stores = text.lines().filter(|l| l.trim_start().starts_with("sta ")).count();
    assert!(stores < 20, "{stores} stores: the loop was unrolled at -O1\n{text}");
}

/// float_in_place_update: a two-trip loop of soft-float calls with a `report` in it, 9441 B at -O1
/// against 19 B at -Os where the trial unroll folds it. The trial now runs at -O1 and keeps it.
#[test]
fn a_float_loop_the_trial_folds_is_unrolled_at_o1() {
    let text = o1("declare void @report(i32)\ndefine i16 @main() {\n  br label %1\n1:\n  \
         %2 = phi double [ 1.000000e+00, %0 ], [ %13, %12 ]\n  %3 = phi double [ 0.000000e+00, %0 ], [ %9, %12 ]\n  \
         %4 = phi double [ undef, %0 ], [ %7, %12 ]\n  %5 = phi i1 [ true, %0 ], [ false, %12 ]\n  \
         %6 = phi i1 [ false, %0 ], [ true, %12 ]\n  %7 = fmul double %2, %2\n  %8 = fdiv double %7, %4\n  \
         %9 = select i1 %5, double %3, double %8\n  %10 = fcmp oeq double %9, 1.000000e+00\n  \
         br i1 %10, label %11, label %12\n11:\n  call void @report(i32 -1)\n  br label %12\n12:\n  \
         %13 = fmul double %2, 5.000000e-01\n  br i1 %6, label %14, label %1\n14:\n  \
         call void @report(i32 7)\n  ret i16 0\n}\n");
    assert!(!text.contains("jsr __"), "the soft-float calls stay: {} lines\n{text}", text.lines().count());
}

/// `-funroll-loops` also turned on peeling at -O1: known_loads_walks grew +4 B (6502) and +47 B
/// (65C02) for 94k clocks fewer. The peeled iteration is a second indirect store (its constant folded).
#[test]
fn an_o1_loop_is_not_peeled() {
    let text = o1("define void @f(ptr %a, i8 %inv, i8 %n) {\nentry:\n  br label %l\nl:\n  \
         %i = phi i8 [0, %entry], [%i1, %l]\n  %p = phi i8 [0, %entry], [%inv, %l]\n  %z = zext i8 %i to i16\n  \
         %q = getelementptr i8, ptr %a, i16 %z\n  %v = load i8, ptr %q\n  %m = mul i8 %v, %p\n  %s = add i8 %m, %p\n  \
         store i8 %s, ptr %q\n  %i1 = add nuw i8 %i, 1\n  %c = icmp ne i8 %i1, %n\n  br i1 %c, label %l, label %x\nx:\n  ret void\n}\n");
    let stores = text.lines().filter(|l| l.trim().starts_with("sta (rc")).count();
    assert_eq!(stores, 1, "the first iteration was peeled\n{text}");
}

/// #9101 made every target's -O1 trial nest-only: float_in_place_update's lone loop stayed rolled,
/// 19 B -> 9441 B. The 6502 trial takes any loop under its size.
#[test]
fn the_6502_unroll_trial_is_not_limited_to_nests() {
    let tti = llrm_6502::driver::machine().tti().expect("a Tti");
    assert!(tti.unroll_trial_threshold().is_some());
    assert!(!tti.unroll_trial_nests_only());
}

/// #9334 gave -Os the x86-m32 size bar, which the 6502 states none of: its -Os trial noted nothing and
/// float_in_memory_loop went 21 B -> 4,988 B, byval_struct 51 B -> 466 B (33 cells, 2026-11-12).
/// float_in_place_update's loop must fold at -Os as it does at -O1.
#[test]
fn a_float_loop_the_trial_folds_is_unrolled_at_os() {
    let ir = format!(
        "target datalayout = \"{}\"\ndeclare void @report(i32)\ndefine i16 @main() {{\n  br label %1\n1:\n  \
         %2 = phi double [ 1.000000e+00, %0 ], [ %13, %12 ]\n  %3 = phi double [ 0.000000e+00, %0 ], [ %9, %12 ]\n  \
         %4 = phi double [ undef, %0 ], [ %7, %12 ]\n  %5 = phi i1 [ true, %0 ], [ false, %12 ]\n  \
         %6 = phi i1 [ false, %0 ], [ true, %12 ]\n  %7 = fmul double %2, %2\n  %8 = fdiv double %7, %4\n  \
         %9 = select i1 %5, double %3, double %8\n  %10 = fcmp oeq double %9, 1.000000e+00\n  \
         br i1 %10, label %11, label %12\n11:\n  call void @report(i32 -1)\n  br label %12\n12:\n  \
         %13 = fmul double %2, 5.000000e-01\n  br i1 %6, label %14, label %1\n14:\n  \
         call void @report(i32 7)\n  ret i16 0\n}}\n",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(Level::Os)).unwrap_or_else(|e| panic!("{e}"));
    assert!(!text.contains("jsr __"), "the soft-float calls stay: {} lines\n{text}", text.lines().count());
}
