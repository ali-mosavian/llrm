target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
define i32 @t(i8 %m) {
entry:
  %r = call { i8, i1 } @llvm.uadd.with.overflow.i8(i8 %m, i8 1)
  %math = extractvalue { i8, i1 } %r, 0
  %ov = extractvalue { i8, i1 } %r, 1
  br i1 %ov, label %exit, label %pre
pre:
  %a = and i8 %math, -8
  br label %loop
loop:
  %niter = phi i8 [ 0, %pre ], [ %n2, %loop ]
  %n2 = add i8 %niter, 8
  %c = icmp eq i8 %n2, %a
  br i1 %c, label %exit, label %loop
exit:
  ret i32 0
}
