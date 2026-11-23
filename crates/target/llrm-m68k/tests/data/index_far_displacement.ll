target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
define i32 @f(ptr %p, i32 %k) {
  %a = getelementptr i8, ptr %p, i32 %k
  %b = getelementptr i8, ptr %a, i32 272
  %v = load i32, ptr %b, align 2
  ret i32 %v
}
