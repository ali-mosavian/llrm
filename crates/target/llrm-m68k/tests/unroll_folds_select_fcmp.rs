//! The unroll analysis simplified neither an `fcmp` nor a `select` of
//! constants, so a two-trip float loop whose compare and `beta` select fold on
//! the trip was priced 15 against its rolled 14 and stayed rolled at -Os:
//! float_in_place_update was 164 B at the 68000, gcc's 16. gcc's
//! complete unroll folds every statement the peeling makes constant.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define i32 @main() {
entry:
  br label %loop
loop:
  %i = phi i32 [1, %entry], [%i1, %latch]
  %beta = phi double [0.0, %entry], [%beta2, %latch]
  %work = phi double [1.0, %entry], [%work2, %latch]
  %oldrho = phi double [0.0, %entry], [%rho, %latch]
  %rho = fmul double %work, %work
  %first = icmp eq i32 %i, 1
  %q = fdiv double %rho, %oldrho
  %beta2 = select i1 %first, double %beta, double %q
  %eq = fcmp oeq double %beta2, 1.0
  br i1 %eq, label %rep, label %latch
rep:
  call void @report(i32 -1)
  br label %latch
latch:
  %work2 = fdiv double %work, 2.0
  %i1 = add i32 %i, 1
  %c = icmp sle i32 %i1, 2
  br i1 %c, label %loop, label %exit
exit:
  call void @report(i32 7)
  ret i32 0
}
";

#[test]
fn a_float_loop_whose_select_and_compare_fold_is_unrolled_at_os() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let text = assembly(IR, cpu, Level::Os).expect("compiles");
        for routine in ["__muldf3", "__divdf3", "__nedf2"] {
            assert!(!text.contains(routine), "{cpu:?}: {routine} left in the loop\n{text}");
        }
    }
}
