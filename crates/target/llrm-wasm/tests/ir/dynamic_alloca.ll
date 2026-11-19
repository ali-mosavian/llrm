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

; The saved stack pointer is used in another block than the save: the copy
; reaches the restore through the loop's blocks.
define i32 @k(i32 %n) {
entry:
  br label %outer
outer:
  %i = phi i32 [ 1, %entry ], [ %next, %latch ]
  %s = call ptr @llvm.stacksave.p0()
  %p = alloca i8, i32 %i, align 16
  br label %inner
inner:
  %j = phi i32 [ 0, %outer ], [ %jn, %inner ]
  %q = getelementptr i8, ptr %p, i32 %j
  store i8 1, ptr %q, align 1
  %jn = add i32 %j, 1
  %jd = icmp eq i32 %jn, %i
  br i1 %jd, label %latch, label %inner
latch:
  call void @llvm.stackrestore.p0(ptr %s)
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %out, label %outer
out:
  ret i32 %i
}
