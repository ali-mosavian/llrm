target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
; bench/recfloat's loop before LoopUnroll: no header phi starts at an integer
; constant (n, 0.0 and 100.0), so the pin runs the extra iterations first.
define i32 @recfloat(i32 %n) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %done, label %loop
loop:
  %i = phi i32 [ %i1, %loop ], [ %n, %entry ]
  %acc = phi double [ %acc1, %loop ], [ 0.0, %entry ]
  %x = phi double [ %x1, %loop ], [ 100.0, %entry ]
  %x1 = tail call double @llvm.fmuladd.f64(double %x, double 5.0e-01, double 1.0)
  %acc1 = fadd double %acc, %x
  %i1 = add nsw i32 %i, -1
  %e = icmp eq i32 %i1, 0
  br i1 %e, label %exit, label %loop
exit:
  %m = tail call double @llvm.fmuladd.f64(double %acc1, double 1.0e+03, double 5.0e-01)
  %r = fptosi double %m to i32
  br label %done
done:
  %res = phi i32 [ 0, %entry ], [ %r, %exit ]
  ret i32 %res
}
declare double @llvm.fmuladd.f64(double, double, double)
