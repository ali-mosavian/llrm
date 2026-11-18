//! A call-free function holding more live 128-bit values than the 24
//! caller-saved vector registers may use `v8`..`v15` (the pin does; the frame
//! saves their low halves). sfm16_float_cmp -O2 spilled its constants and
//! temporaries inside the hot loop: 3,389 instructions against clang's 3,263.
//! A function that calls keeps them out: a call would drop the upper halves.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const LIVE: usize = 28;

fn kernel(calls: bool) -> String {
    let mut ir = String::from(
        "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
         target triple = \"aarch64-unknown-linux-gnu\"\n\
         declare void @ext()\n\
         define void @f(ptr %p, i64 %n, ptr %out) {\nentry:\n  br label %loop\nloop:\n  %i = phi i64 [ 0, %entry ], [ %next, %loop ]\n",
    );
    for k in 0..LIVE {
        ir += &format!("  %a{k} = phi <4 x i32> [ zeroinitializer, %entry ], [ %n{k}, %loop ]\n");
    }
    for k in 0..LIVE {
        ir += &format!(
            "  %g{k} = getelementptr <4 x i32>, ptr %p, i64 {}\n  %l{k} = load <4 x i32>, ptr %g{k}, align 16\n  %n{k} = add <4 x i32> %a{k}, %l{k}\n",
            k + 1
        );
    }
    if calls {
        ir += "  call void @ext()\n";
    }
    ir += "  %next = add i64 %i, 1\n  %done = icmp eq i64 %next, %n\n  br i1 %done, label %exit, label %loop\nexit:\n";
    let mut sum = String::from("%n0");
    for k in 1..LIVE {
        ir += &format!("  %s{k} = add <4 x i32> {sum}, %n{k}\n");
        sum = format!("%s{k}");
    }
    ir + &format!("  store <4 x i32> {sum}, ptr %out, align 16\n  ret void\n}}\n")
}

fn uses_callee_saved_vectors(asm: &str) -> bool {
    (8..=15).any(|n| asm.contains(&format!("v{n}.4s")) || asm.contains(&format!("q{n},")))
}

#[test]
fn a_leaf_with_many_live_vectors_uses_v8_to_v15() {
    let asm = driver::assembly(&kernel(false), Platform::Linux, Level::O2).expect("lowers");
    assert!(uses_callee_saved_vectors(&asm), "{asm}");
}

#[test]
fn a_function_that_calls_keeps_v8_to_v15_out() {
    let asm = driver::assembly(&kernel(true), Platform::Linux, Level::O2).expect("lowers");
    assert!(!uses_callee_saved_vectors(&asm), "{asm}");
}
