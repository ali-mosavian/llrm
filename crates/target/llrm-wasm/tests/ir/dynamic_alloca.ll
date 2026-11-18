target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @use(ptr)
define void @f(i32 %n) {
  %p = alloca i32, i32 %n, align 4
  call void @use(ptr %p)
  ret void
}
define void @g(i32 %n) {
  %s = call ptr @llvm.stacksave.p0()
  %p = alloca i32, i32 %n, align 4
  call void @use(ptr %p)
  call void @llvm.stackrestore.p0(ptr %s)
  ret void
}
declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)

; The alloca in a loop that calls: the stack pointer is written back after
; each call-frame bracket, and the saved value is read before the alloca.
define i32 @h(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %s = call ptr @llvm.stacksave.p0()
  %p = alloca i32, i32 %i, align 4
  call void @use(ptr %p)
  call void @llvm.stackrestore.p0(ptr %s)
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %out, label %loop
out:
  ret i32 %i
}
