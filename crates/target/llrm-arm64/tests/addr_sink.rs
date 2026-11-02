//! Address-mode sinking (`CodeGenPrepare::optimizeMemoryInst`) on arm64.

use llrm_arm64::driver::{self, IselMode};
use llrm_arm64::machine::Platform;

/// The scaled-index mode was judged for an 8-byte access, so a float's
/// `[base, idx, lsl #2]` was "illegal" and the address stayed in the block
/// that made it: `add` per trip where the pin folds it into the load
/// (`x_nbody_f` -O1 +1500 instructions).
#[test]
fn a_float_index_is_folded_into_a_load_in_another_block() {
    let ir = "target datalayout = \"e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32\"\n\
@a = global [64 x float] zeroinitializer\n\
define float @f(i64 %i) {\n\
entry:\n  %p = getelementptr inbounds float, ptr @a, i64 %i\n  br label %loop\n\
loop:\n  %k = phi i64 [ 0, %entry ], [ %k1, %loop ]\n  %v = load volatile float, ptr %p\n  %k1 = add i64 %k, 1\n\
  %d = icmp ult i64 %k1, 100\n  br i1 %d, label %loop, label %exit\n\
exit:\n  ret float %v\n}\n";
    driver::set_isel_mode(IselMode::Dag);
    let asm = driver::assembly(ir, Platform::Darwin, true).expect("lowers");
    assert!(asm.contains("lsl #2]"), "{asm}");
}
