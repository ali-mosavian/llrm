target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
declare void @report(i64)

define void @f(i32 %k) {
entry:
  %a = alloca [50 x i32], align 16
  %s = call ptr @llvm.stacksave.p0()
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %n, %loop ]
  %p = getelementptr i32, ptr %a, i32 %i
  %v = mul i32 %i, %k
  store volatile i32 %v, ptr %p
  %n = add i32 %i, 1
  %c = icmp ult i32 %n, 50
  br i1 %c, label %loop, label %done

done:
  call void @llvm.stackrestore.p0(ptr %s)
  call void @report(i64 1)
  call void @report(i64 2)
  ret void
}

define void @g(i32 %k, i32 %m) {
entry:
  %s = call ptr @llvm.stacksave.p0()
  %a = alloca i32, i32 %m, align 16
  store volatile i32 %k, ptr %a
  call void @llvm.stackrestore.p0(ptr %s)
  call void @report(i64 1)
  call void @report(i64 2)
  ret void
}
