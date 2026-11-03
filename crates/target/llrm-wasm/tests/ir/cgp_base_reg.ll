target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@g = external global [64 x float], align 16

define float @f(i32 %i, i1 %c) {
a:
  %p = getelementptr i8, ptr @g, i32 %i
  %q = getelementptr i8, ptr %p, i32 4
  %x = load float, ptr %q, align 4
  br i1 %c, label %b, label %d
b:
  %r = getelementptr i8, ptr %p, i32 8
  %y = load float, ptr %r, align 4
  %z = fadd float %x, %y
  ret float %z
d:
  ret float %x
}
