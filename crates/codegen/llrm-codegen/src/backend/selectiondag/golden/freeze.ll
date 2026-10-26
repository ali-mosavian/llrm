target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

define i32 @f(i32 %x) {
  %y = freeze i32 %x
  %z = add i32 %y, %y
  ret i32 %z
}

define i32 @g() {
  %y = freeze i32 undef
  ret i32 %y
}

define i32 @h() {
  %y = freeze i32 7
  %z = freeze i32 %y
  ret i32 %z
}
