target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare i32 @callee(i32, i32)
declare void @sink(i32)

define i32 @f(i32 %a, i16 %b) {
  %c = add i32 %a, 1
  %r = call i32 @callee(i32 %c, i32 7)
  %s = add i32 %r, %c
  ret i32 %s
}

define void @g(i32 %a) {
  call void @sink(i32 %a)
  ret void
}
