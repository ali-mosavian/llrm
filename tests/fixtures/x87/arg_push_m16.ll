target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"
target triple = "i386-unknown-unknown-code16"

declare void @g(i16, float)

define void @f(i16 %a, float %x) {
  %y = fmul float %x, %x
  call void @g(i16 %a, float %y)
  ret void
}
