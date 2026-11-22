//! A computed goto the front end factored into one dispatch block is copied
//! back into every handler, as the pin's post-RA tail duplication does.

use llrm_arm64::driver::{self, Level};
use llrm_arm64::machine::Platform;

const THREADED: &str =
    "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
target triple = \"aarch64-unknown-linux-gnu\"
@table = internal constant [4 x ptr] [ptr blockaddress(@run, %a), ptr blockaddress(@run, %b), ptr blockaddress(@run, %c), ptr blockaddress(@run, %done)]
define i64 @run(ptr %pc0, i64 %acc0) {
entry:
  br label %dispatch
dispatch:
  %pc = phi ptr [ %pc0, %entry ], [ %pc.a, %a ], [ %pc.b, %b ], [ %pc.c, %c ]
  %acc = phi i64 [ %acc0, %entry ], [ %acc.a, %a ], [ %acc.b, %b ], [ %acc.c, %c ]
  %op = load i8, ptr %pc, align 1
  %next = getelementptr i8, ptr %pc, i64 1
  %ix = zext i8 %op to i64
  %slot = getelementptr [4 x ptr], ptr @table, i64 0, i64 %ix
  %dest = load ptr, ptr %slot, align 8
  indirectbr ptr %dest, [label %a, label %b, label %c, label %done]
a:
  %acc.a = mul i64 %acc, 3
  %pc.a = getelementptr i8, ptr %next, i64 0
  br label %dispatch
b:
  %acc.b = xor i64 %acc, 5
  %pc.b = getelementptr i8, ptr %next, i64 0
  br label %dispatch
c:
  %acc.c = lshr i64 %acc, 2
  %pc.c = getelementptr i8, ptr %next, i64 0
  br label %dispatch
done:
  ret i64 %acc
}
";

/// An interpreter's `goto *table[*pc++]` was one shared `br x` (a 20-handler
/// VM at -O2: 1 `br` against the pin's 20): every handler paid a branch to the
/// dispatch and the one indirect branch predicted badly.
#[test]
fn a_factored_computed_goto_is_copied_into_each_handler() {
    for level in [Level::O2, Level::Os] {
        let asm = driver::assembly(THREADED, Platform::Linux, level).expect("lowers");
        assert!(
            jumps(&asm) >= 4,
            "{level:?}: {} indirect jumps, want one per handler and the entry:\n{asm}",
            jumps(&asm)
        );
    }
}

fn jumps(asm: &str) -> usize {
    asm.lines().filter(|line| line.trim_start().starts_with("br ")).count()
}
