target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"
target triple = "i386-unknown-unknown-code16"
declare void @g(ptr, float)
declare void @h(ptr)
define void @f(ptr %o, ptr %p, float %a, float %b, float %d, float %e) {
  %hp = getelementptr inbounds i8, ptr %p, i16 24
  %h = load i16, ptr %hp, align 1
  %lt = icmp slt i16 %h, 1
  br i1 %lt, label %dead, label %live
dead:
  %lt2 = icmp slt i16 %h, -40
  br i1 %lt2, label %d1, label %d2
d1:
  call void @g(ptr %o, float %e)
  ret void
d2:
  call void @h(ptr %o)
  ret void
live:
  %m = fmul float %a, %b
  %n = fmul float %d, %e
  %s2 = fadd float %m, %n
  call void @g(ptr %o, float %s2)
  ret void
}
