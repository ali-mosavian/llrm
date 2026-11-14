target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
; b[i] = clamp(a[i] + 128.5f, 0, 255) as u8: fadd, two fcmp, two select, fptosi, trunc
define void @f(ptr noalias %a, ptr noalias %b) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %pa = getelementptr inbounds float, ptr %a, i64 %i
  %x = load float, ptr %pa, align 4
  %r = fadd float %x, 1.285000e+02
  %lo = fcmp ole float %r, 0.000000e+00
  %hi = fcmp oge float %r, 2.550000e+02
  %c = select i1 %hi, float 2.550000e+02, float %r
  %n = fptosi float %c to i32
  %t = trunc i32 %n to i8
  %o = select i1 %lo, i8 0, i8 %t
  %pb = getelementptr inbounds i8, ptr %b, i64 %i
  store i8 %o, ptr %pb, align 1
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 8
  br i1 %done, label %exit, label %loop
exit:
  ret void
}
