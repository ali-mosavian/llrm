target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_a = global [256 x i32] zeroinitializer, align 4

define void @f(i32 %0) {
  %2 = icmp sgt i32 %0, 1
  br i1 %2, label %3, label %22

3:
  br label %4

4:
  %5 = phi i32 [ %20, %17 ], [ 1, %3 ]
  %6 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %5
  %7 = load i32, ptr %6, align 4
  br label %8

8:
  %9 = phi i32 [ %5, %4 ], [ %10, %14 ]
  %10 = add nsw i32 %9, -1
  %11 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %10
  %12 = load i32, ptr %11, align 4
  %13 = icmp sgt i32 %12, %7
  br i1 %13, label %14, label %17

14:
  %15 = getelementptr inbounds nuw [4 x i8], ptr @_a, i32 %9
  store i32 %12, ptr %15, align 4
  %16 = icmp sgt i32 %9, 1
  br i1 %16, label %8, label %17

17:
  %18 = phi i32 [ 0, %14 ], [ %9, %8 ]
  %19 = getelementptr inbounds [4 x i8], ptr @_a, i32 %18
  store i32 %7, ptr %19, align 4
  %20 = add nuw nsw i32 %5, 1
  %exitcond = icmp eq i32 %20, %0
  br i1 %exitcond, label %21, label %4

21:
  br label %22

22:
  ret void
}
