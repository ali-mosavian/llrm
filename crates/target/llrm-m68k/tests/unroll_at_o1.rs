//! clang marks every loop `unroll.disable` at -O1, so a two-trip loop of float
//! updates that the first trips fold away stayed rolled: float_in_place_update
//! was 170 B at the 68000 (a `__muldf3`, `__divdf3` and `__nedf2` call per trip)
//! where gcc's complete unroll leaves one `report` call (16 B). The m68k rows
//! take `-funroll-loops` at -O1; with the pin's threshold alone that unrolled
//! every small loop (net +3376 B over the corpus: 70 cells larger), so -O1
//! unrolls only where the copies cost no more than the rolled loop
//! (`unroll_never_grows_at_o1`, gcc's size-neutral complete unroll).

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::compile::CodeModel;
use llrm_m68k::driver::{Level, Policy, assembly_flagged};

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
fn a_two_trip_float_loop_that_folds_is_unrolled_at_o1() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        // `-funroll-loops` is what the m68k rows of `llrm-cc` pass at -O1 (the loop carries no
        // `unroll.disable` then)
        let mut policy = Policy::default();
        assert!(policy.take("-funroll-loops"));
        let text = assembly_flagged(IR, cpu, Level::O1, false, None, CodeModel::Small, &policy).expect("compiles");
        for routine in ["__muldf3", "__divdf3", "__nedf2"] {
            assert!(!text.contains(routine), "{cpu:?}: {routine} left in the loop\n{text}");
        }
    }
}

const CALLS: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @report(i32)
define void @f() {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%n, %loop]
  call void @report(i32 %i)
  %n = add i32 %i, 1
  %c = icmp ult i32 %n, 4
  br i1 %c, label %loop, label %exit
exit:
  ret void
}
";

/// Four copies of a call are larger than the loop around one and save its counter and branch:
/// -O1 is judged by clocks, so the pin's threshold unrolls it.
#[test]
fn a_loop_whose_copies_cost_more_bytes_is_unrolled_at_o1_for_its_clocks() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let mut policy = Policy::default();
        assert!(policy.take("-funroll-loops"));
        let text = assembly_flagged(CALLS, cpu, Level::O1, false, None, CodeModel::Small, &policy).expect("compiles");
        assert_eq!(text.matches("report").count(), 4, "{cpu:?}: stayed rolled\n{text}");
    }
}
