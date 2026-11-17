target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)
declare void @report(i64)
declare i32 @vla(i32)

define i64 @f() {
  br label %2

1:
  ret i64 %23

2:
  %3 = phi i64 [ 1, %0 ], [ %24, %10 ]
  %4 = phi i64 [ 3, %0 ], [ %25, %10 ]
  %5 = phi i64 [ 0, %0 ], [ %23, %10 ]
  %6 = phi i64 [ 0, %0 ], [ %14, %10 ]
  %7 = mul nuw nsw i64 %3, 3
  %8 = call ptr @llvm.stacksave.p0()
  %9 = alloca i8, i64 %7, align 1
  br label %26

10:
  %11 = ptrtoint ptr %9 to i64
  %12 = sub nsw i64 %11, %6
  call void @report(i64 noundef %12)
  %13 = icmp eq i64 %6, 0
  %14 = select i1 %13, i64 %11, i64 %6
  %15 = getelementptr i8, ptr %9, i64 %7
  %16 = getelementptr i8, ptr %15, i64 -1
  %17 = load i8, ptr %16, align 1
  %18 = zext i8 %17 to i32
  %19 = trunc nuw nsw i64 %3 to i32
  %20 = call i32 @vla(i32 noundef %19)
  %21 = add nuw nsw i32 %20, %18
  %22 = zext nneg i32 %21 to i64
  %23 = add nuw nsw i64 %5, %22
  call void @llvm.stackrestore.p0(ptr %8)
  %24 = add nuw nsw i64 %3, 1
  %25 = add nuw nsw i64 %4, 3
  %exitcond1 = icmp eq i64 %24, 7
  br i1 %exitcond1, label %1, label %2

26:
  %27 = phi i64 [ 0, %2 ], [ %31, %26 ]
  %28 = add i64 %3, %27
  %29 = trunc i64 %28 to i8
  %30 = getelementptr i8, ptr %9, i64 %27
  store i8 %29, ptr %30, align 1
  %31 = add nuw nsw i64 %27, 1
  %exitcond = icmp eq i64 %4, %31
  br i1 %exitcond, label %10, label %26
}
