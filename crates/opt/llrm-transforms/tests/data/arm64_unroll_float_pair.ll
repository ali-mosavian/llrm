target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define void @shape(ptr %p, float %a, float %b) {
entry:
  br label %l
l:
  %k = phi i32 [ 0, %entry ], [ %k1, %l ]
  %pk = getelementptr float, ptr %p, i32 %k
  %v = load float, ptr %pk
  %m0 = fmul float %v, %a
  %s0 = fadd float %m0, %b
  %m1 = fmul float %s0, %a
  %s1 = fadd float %m1, %b
  store float %s1, ptr %pk
  %k1 = add i32 %k, 1
  %c = icmp ne i32 %k1, 6
  br i1 %c, label %l, label %x
x:
  ret void
}
