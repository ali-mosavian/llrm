target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare void @llvm.experimental.memset.pattern.p0.i16.i32(ptr, i16, i32, i1)

@s = global [80 x i16] zeroinitializer

define void @constant_length() {
  call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr @s, i16 1824, i32 80, i1 false)
  ret void
}

define void @variable_length(ptr %p, i32 %n) {
  call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr %p, i16 1824, i32 %n, i1 false)
  ret void
}

define i32 @tail(ptr %p, i32 %n, i16 %v) {
  call void @llvm.experimental.memset.pattern.p0.i16.i32(ptr %p, i16 %v, i32 %n, i1 false)
  %r = add i32 %n, 1
  ret i32 %r
}
