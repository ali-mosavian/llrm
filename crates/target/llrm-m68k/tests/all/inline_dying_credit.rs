//! jpeg_fixed -Os (68000/68020, +246/+288 B against gcc): `message`, a loop and
//! a call, private and called from ten places was copied into each, the units
//! crediting the callee that dies with its last call. The copies were 20 B each
//! against a 14 B call. A speed caller keeps the credit.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const DATALAYOUT: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"\n";

const MSG: &str = "declare void @write(i32, ptr, i32)\n\
define internal void @msg(ptr %t) {\nentry:\n  br label %loop\nloop:\n\
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]\n  %p = getelementptr i8, ptr %t, i32 %i\n  %c = load i8, ptr %p\n\
  %n = add i32 %i, 1\n  %z = icmp eq i8 %c, 0\n  br i1 %z, label %done, label %loop\ndone:\n\
  call void @write(i32 1, ptr %t, i32 %i)\n  ret void\n}\n";

fn ten_sites() -> String {
    // the strings are read from memory: with constants an inline folds the loop
    // away and is faster, which the -Os trial keeps (the loop priced once
    // for ten calls hid that, `trial::price_of_functions`)
    let globals = "@s = external global [10 x ptr]\n";
    let calls: String = (0..10)
        .map(|i| format!("  %g{i} = getelementptr ptr, ptr @s, i32 {i}\n  %p{i} = load ptr, ptr %g{i}\n  call void @msg(ptr %p{i})\n"))
        .collect();
    format!("{DATALAYOUT}{globals}{MSG}define void @main() optsize nounwind {{\n{calls}  ret void\n}}\n")
}

/// Ten copies of the loop are not smaller than one loop and ten calls.
#[test]
fn a_private_callee_with_ten_sites_stays_a_call_at_size() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(&ten_sites(), cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(asm.contains("msg:"), "{cpu}: the callee is gone, copied into its sites:\n{asm}");
    }
}

/// A call in a loop at -O2 is a copy the speed caller wants: the credit stays.
#[test]
fn a_callee_of_a_hot_loop_is_still_copied_at_speed() {
    let ir = format!(
        "{DATALAYOUT}{MSG}@s = private constant [4 x i8] c\"abc\\00\"\n\
         define void @main(i32 %k) nounwind {{\nentry:\n  br label %l\nl:\n  %j = phi i32 [ 0, %entry ], [ %j2, %l ]\n\
         \x20 call void @msg(ptr @s)\n  call void @msg(ptr @s)\n  %j2 = add i32 %j, 1\n  %d = icmp eq i32 %j2, %k\n\
         \x20 br i1 %d, label %x, label %l\nx:\n  ret void\n}}\n"
    );
    let asm = assembly(&ir, cpu_of("68020").unwrap(), Level::O2).expect("compiles");
    assert!(!asm.contains("msg:"), "the callee stayed outlined at -O2:\n{asm}");
}

/// shift_int64_masked_amount -Os (`tests/ir/hot_callee_at_size.ll`, the
/// program's IR): `mix`, three calls in a nested loop, stayed a call at -Os
/// (188,470 -> 276,962 clocks for -6 B). A hot site keeps the credit.
#[test]
fn a_callee_of_a_hot_loop_is_still_copied_at_size() {
    let ir = include_str!("../ir/hot_callee_at_size.ll");
    for cpu in ["68000", "68020"] {
        let asm = assembly(ir, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        assert!(!asm.contains("jsr\t(mix"), "{cpu}: the hot callee stayed a call at -Os:\n{asm}");
    }
}
