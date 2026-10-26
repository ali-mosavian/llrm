target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

@fp = global ptr null

define i32 @viaarg(ptr %f, i32 %a) {
  %r = call i32 %f(i32 %a)
  ret i32 %r
}

define void @viaload(i32 %a) {
  %f = load ptr, ptr @fp
  call void %f(i32 %a)
  ret void
}
