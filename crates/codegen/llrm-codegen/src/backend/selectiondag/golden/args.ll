target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

define i32 @f(i32 %a, i16 %b) {
  %c = add i32 %a, 1
  ret i32 %c
}

define i32 @h(i32 %a, i32 %b) {
e:
  %s = add i32 %a, %b
  br label %n
n:
  %t = mul i32 %s, %a
  ret i32 %t
}

define void @v(ptr %p) {
  store i32 1, ptr %p
  ret void
}
