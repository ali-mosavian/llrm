target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
declare void @report(i32)

define void @f() {
entry:
  br label %loop

loop:
  %k = phi i32 [ 1, %entry ], [ %k2, %loop ]
  %sp = call ptr @llvm.stacksave.p0()
  %buf = alloca i8, i32 %k, align 1
  store i8 7, ptr %buf, align 1
  %v = load i8, ptr %buf, align 1
  %w = zext i8 %v to i32
  call void @llvm.stackrestore.p0(ptr %sp)
  call void @report(i32 %w)
  %k2 = add nuw nsw i32 %k, 1
  %done = icmp eq i32 %k2, 7
  br i1 %done, label %exit, label %loop

exit:
  ret void
}
