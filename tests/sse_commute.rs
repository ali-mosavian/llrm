//! A scalar SSE add or multiply is commutable (`isCommutable` on `ADDSS`,
//! `ADDSD`, `MULSS`, `MULSD`), so `fadd(product, acc)` can tie the accumulator.
//! `commutative` knew only the integer mnemonics: the contracted sum tied the
//! product and each loop-carried accumulator paid a `movaps xmm0, xmm1` per
//! trip (fpbench +2 instructions, nbody_single +3.5% executed).

mod common;

use std::process::Command;

const IR: &str = "target datalayout = \"@LAYOUT@\"
target triple = \"x86_64-unknown-linux-gnu\"

define float @sum(ptr %p, i64 %n, i64 %skip) {
entry:
  br label %head
head:
  %i = phi i64 [ 0, %entry ], [ %next, %join ]
  %acc = phi float [ 0.0, %entry ], [ %out, %join ]
  %same = icmp eq i64 %i, %skip
  br i1 %same, label %join, label %body
body:
  %at = getelementptr float, ptr %p, i64 %i
  %x = load float, ptr %at
  %y = fadd float %x, 1.0
  %fused = tail call float @llvm.fmuladd.f32(float %x, float %y, float %acc)
  br label %join
join:
  %out = phi float [ %fused, %body ], [ %acc, %head ]
  %next = add i64 %i, 1
  %more = icmp eq i64 %next, %n
  br i1 %more, label %done, label %head
done:
  ret float %out
}

declare float @llvm.fmuladd.f32(float, float, float)
";

#[test]
fn a_loop_carried_float_sum_ties_the_accumulator() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("f.ll");
    std::fs::write(&input, IR.replace("@LAYOUT@", &common::datalayout("x86-m64"))).unwrap();
    let done =
        Command::new(env!("CARGO_BIN_EXE_llrm-llc")).args(["-O2", "-S", "-o", "-"]).arg(&input).output().unwrap();
    assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    let asm = String::from_utf8_lossy(&done.stdout);
    assert!(asm.contains("addss xmm0, xmm") && !asm.contains("movaps xmm0, xmm"), "{asm}");
}
