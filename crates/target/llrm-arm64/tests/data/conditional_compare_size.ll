; A scan with a compare chain whose later compares the trace model finds too
; late (`and`/`or` of `== 34`, `== 40`, `== 41` over a loaded byte).
target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

@sg = external global [1024 x i8]

declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define i32 @paren_size() nounwind optsize {
  %1 = alloca [16 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @sg, i64 604), align 4
  %wide.trip.count = zext i16 %2 to i64
  br label %3

3:
  %4 = phi i64 [ %29, %27 ], [ 0, %0 ]
  %5 = phi i32 [ 0, %0 ], [ %28, %27 ]
  %6 = phi i1 [ false, %0 ], [ %14, %27 ]
  %exitcond = icmp eq i64 %wide.trip.count, %4
  br i1 %exitcond, label %7, label %9

7:
  %8 = icmp eq i32 %5, 0
  br i1 %8, label %35, label %30

9:
  %10 = getelementptr i8, ptr @sg, i64 %4
  %11 = load i8, ptr %10, align 1
  %12 = zext i8 %11 to i32
  %13 = icmp eq i32 %12, 34
  %14 = xor i1 %6, %13
  br i1 %14, label %27, label %15

15:
  %16 = icmp eq i32 %12, 40
  %17 = icmp ult i32 %5, 16
  %or.cond = select i1 %16, i1 %17, i1 false
  br i1 %or.cond, label %18, label %22

18:
  %19 = zext nneg i32 %5 to i64
  %20 = add nuw nsw i32 %5, 1
  %21 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %19
  %tmp = trunc i64 %4 to i32
  store i32 %tmp, ptr %21, align 4
  br label %27

22:
  %23 = icmp eq i32 %12, 41
  %24 = icmp ne i32 %5, 0
  %or.cond1 = select i1 %23, i1 %24, i1 false
  %25 = sext i1 %or.cond1 to i32
  %26 = add i32 %5, %25
  br label %27

27:
  %28 = phi i32 [ %5, %9 ], [ %20, %18 ], [ %26, %22 ]
  %29 = add nuw nsw i64 %4, 1
  br label %3

30:
  %31 = add i32 %5, -1
  %32 = zext i32 %31 to i64
  %33 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %32
  %34 = load i32, ptr %33, align 4
  br label %35

35:
  %36 = phi i32 [ %34, %30 ], [ -1, %7 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret i32 %36
}

define i32 @paren_speed() nounwind {
  %1 = alloca [16 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @sg, i64 604), align 4
  %wide.trip.count = zext i16 %2 to i64
  br label %3

3:
  %4 = phi i64 [ %29, %27 ], [ 0, %0 ]
  %5 = phi i32 [ 0, %0 ], [ %28, %27 ]
  %6 = phi i1 [ false, %0 ], [ %14, %27 ]
  %exitcond = icmp eq i64 %wide.trip.count, %4
  br i1 %exitcond, label %7, label %9

7:
  %8 = icmp eq i32 %5, 0
  br i1 %8, label %35, label %30

9:
  %10 = getelementptr i8, ptr @sg, i64 %4
  %11 = load i8, ptr %10, align 1
  %12 = zext i8 %11 to i32
  %13 = icmp eq i32 %12, 34
  %14 = xor i1 %6, %13
  br i1 %14, label %27, label %15

15:
  %16 = icmp eq i32 %12, 40
  %17 = icmp ult i32 %5, 16
  %or.cond = select i1 %16, i1 %17, i1 false
  br i1 %or.cond, label %18, label %22

18:
  %19 = zext nneg i32 %5 to i64
  %20 = add nuw nsw i32 %5, 1
  %21 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %19
  %tmp = trunc i64 %4 to i32
  store i32 %tmp, ptr %21, align 4
  br label %27

22:
  %23 = icmp eq i32 %12, 41
  %24 = icmp ne i32 %5, 0
  %or.cond1 = select i1 %23, i1 %24, i1 false
  %25 = sext i1 %or.cond1 to i32
  %26 = add i32 %5, %25
  br label %27

27:
  %28 = phi i32 [ %5, %9 ], [ %20, %18 ], [ %26, %22 ]
  %29 = add nuw nsw i64 %4, 1
  br label %3

30:
  %31 = add i32 %5, -1
  %32 = zext i32 %31 to i64
  %33 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %32
  %34 = load i32, ptr %33, align 4
  br label %35

35:
  %36 = phi i32 [ %34, %30 ], [ -1, %7 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret i32 %36
}
