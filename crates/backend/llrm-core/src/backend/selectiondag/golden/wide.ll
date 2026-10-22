target datalayout = "e-m:e-p:32:32-i64:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
@g = global i64 0
declare i64 @callee(i64, i32)
define i64 @f(i64 %a, i32 %b) {
e:
  %x = load i64, ptr @g
  %s = add i64 %a, %x
  br label %n
n:
  %t = add i64 %s, 5
  %r = call i64 @callee(i64 %t, i32 %b)
  store i64 %r, ptr @g
  ret i64 %r
}

define i64 @id(i64 %a) {
  ret i64 %a
}

define void @store(i64 %a, ptr %p) {
  %s = add i64 %a, 1
  store i64 %s, ptr %p
  ret void
}
