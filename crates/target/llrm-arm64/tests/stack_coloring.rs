//! `StackColoring` on arm64: static allocas whose lifetimes never overlap share
//! one frame object.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const HEADER: &str = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n\
declare void @use(ptr)\n\
declare void @llvm.lifetime.start.p0(i64, ptr)\n\
declare void @llvm.lifetime.end.p0(i64, ptr)\n";

/// The bytes the prologue reserves, from its `sub sp` or pre-indexed `stp`.
fn frame(asm: &str) -> u32 {
    asm.lines()
        .filter_map(|line| {
            let line = line.trim();
            let number = line.strip_prefix("sub sp, sp, #").or_else(|| line.split("[sp, #-").nth(1))?;
            number.trim_end_matches("]!").parse().ok()
        })
        .max()
        .unwrap_or(0)
}

/// Two `[64 x i32]` locals: the first ends before the second starts, or both
/// are live at the second's use.
fn two_locals(disjoint: bool) -> String {
    let mark = |what: &str, local: &str| format!("  call void @llvm.lifetime.{what}.p0(i64 256, ptr %{local})\n");
    let call = |local: &str| format!("  call void @use(ptr %{local})\n");
    let mut body =
        String::from("define void @f() {\n  %a = alloca [64 x i32], align 4\n  %b = alloca [64 x i32], align 4\n");
    body += &(mark("start", "a") + &call("a"));
    if disjoint {
        body += &mark("end", "a");
    }
    body += &(mark("start", "b") + &call("b"));
    if !disjoint {
        body += &(call("a") + &mark("end", "a"));
    }
    body += &(mark("end", "b") + "  ret void\n}\n");
    format!("{HEADER}{body}")
}

/// `suggest.c` -Os `sugg_find` (31 locals, 71 lifetime markers) kept every
/// local in its own frame object: 2,448 B against the pin's 1,600. Two `[64 x
/// i32]` that are never live together were a 512-byte frame; they are one
/// 256-byte object.
#[test]
fn locals_never_live_together_share_a_frame_object() {
    let asm = driver::assembly(&two_locals(true), Platform::Linux, Level::O1).expect("lowers");
    assert!(frame(&asm) <= 300, "{asm}");
}

/// Two locals live together keep an object each.
#[test]
fn locals_live_together_keep_their_own_objects() {
    let asm = driver::assembly(&two_locals(false), Platform::Linux, Level::O1).expect("lowers");
    assert!(frame(&asm) >= 512, "{asm}");
}
