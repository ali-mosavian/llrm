//! Stack arrays filled in a loop that calls the soft-float routines took one
//! pointer register each (`move.l %d0, (%a3)+`, `move.l %d0, (%a2)+`), because
//! LSR counted every `alloca` base as a register. A stack slot is `(d,%sp,Xn)`,
//! no register: the arrays share one index register and the callee-saved
//! address registers (a2-a5) stay free for the call targets (fpbench -22 B,
//! nbody -94 B at 68000 -Os).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define float @k(i32 %n) optsize {
  %a = alloca [6 x float], align 4
  %b = alloca [6 x float], align 4
  %c = alloca [6 x float], align 4
  %d = alloca [6 x float], align 4
  br label %loop
loop:
  %i = phi i32 [ 0, %0 ], [ %next, %loop ]
  %m7 = mul nuw nsw i32 %i, 7
  %x7 = add nsw i32 %m7, -15
  %f7 = sitofp i32 %x7 to float
  %pa = getelementptr inbounds nuw [4 x i8], ptr %a, i32 %i
  store float %f7, ptr %pa, align 4
  %m5 = mul nuw nsw i32 %i, 5
  %x5 = add nsw i32 %m5, -12
  %f5 = sitofp i32 %x5 to float
  %pb = getelementptr inbounds nuw [4 x i8], ptr %b, i32 %i
  store float %f5, ptr %pb, align 4
  %pc = getelementptr inbounds nuw [4 x i8], ptr %c, i32 %i
  store float 0.0, ptr %pc, align 4
  %pd = getelementptr inbounds nuw [4 x i8], ptr %d, i32 %i
  store float 0.0, ptr %pd, align 4
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, 6
  br i1 %done, label %out, label %loop, !llvm.loop !0
out:
  %ra = getelementptr inbounds [4 x i8], ptr %a, i32 %n
  %rb = getelementptr inbounds [4 x i8], ptr %b, i32 %n
  %va = load float, ptr %ra, align 4
  %vb = load float, ptr %rb, align 4
  %s = fadd float %va, %vb
  ret float %s
}

!0 = distinct !{!0, !1}
!1 = !{!"llvm.loop.unroll.disable"}
"#;

#[test]
fn stack_arrays_in_a_loop_share_one_index_register_not_one_pointer_each() {
    for cpu in ["68000", "68020"] {
        for level in [Level::Os, Level::O2] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let walks: Vec<&str> = out.lines().filter(|l| l.contains(")+") && !l.contains("%sp")).collect();
            assert!(walks.is_empty(), "{cpu}: a pointer walk per stack array:\n{out}");
        }
    }
}
