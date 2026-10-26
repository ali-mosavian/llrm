target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare void @use(ptr)

define void @dyn(i32 %n) {
  %a = alloca i8, i32 %n
  call void @use(ptr %a)
  ret void
}

define void @dyn4(i32 %n) {
  %a = alloca i32, i32 %n, align 16
  call void @use(ptr %a)
  ret void
}

define void @late(i32 %n) {
entry:
  %c = icmp eq i32 %n, 0
  br i1 %c, label %out, label %in
in:
  %a = alloca i32
  call void @use(ptr %a)
  br label %out
out:
  ret void
}
