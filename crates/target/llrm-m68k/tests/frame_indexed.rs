//! A stack array read at an index after a loop was addressed through a `lea`
//! of its frame slot, re-formed for every access because a call destroys the
//! register (`lea (4,%sp), %a0; move.l (0,%a0,%d2), -(%sp)`, 4 B and 8 clk
//! each): `move.l (4,%sp,%d2), -(%sp)` takes the stack pointer as the base.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define float @k(i32 %n) optsize {
  %a = alloca [6 x float], align 4
  %b = alloca [6 x float], align 4
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
fn a_stack_array_indexed_by_a_register_is_addressed_from_the_stack_pointer() {
    for cpu in ["68000", "68020"] {
        for level in [Level::Os, Level::O2] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).expect("compiles");
            let lines: Vec<&str> = out.lines().collect();
            for (at, line) in lines.iter().enumerate() {
                let Some(rest) = line.strip_prefix("\tlea\t(") else { continue };
                if !rest.contains(",%sp), %a") {
                    continue;
                }
                let register = rest.rsplit(' ').next().unwrap();
                let base = format!("(0,{register},");
                assert!(
                    !lines[at + 1..].iter().take(3).any(|next| next.contains(&base)),
                    "{cpu} {level:?}: a lea of a stack slot feeds an indexed operand:\n{out}"
                );
            }
            assert!(out.contains(",%sp,%d"), "{cpu} {level:?}: no (d,%sp,Xn) operand:\n{out}");
        }
    }
}
