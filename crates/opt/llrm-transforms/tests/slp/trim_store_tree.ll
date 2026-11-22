target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@A = internal global [12 x [12 x double]] zeroinitializer, align 8

define void @f() {
  br label %1

1:
  %2 = phi i64 [ 0, %0 ], [ %7, %1 ]
  %3 = getelementptr inbounds nuw [96 x i8], ptr @A, i64 %2
  %4 = trunc nuw nsw i64 %2 to i32
  %5 = uitofp nneg i32 %4 to double
  %6 = fmul double %5, 5.000000e-01
  store double %6, ptr %3, align 8
  %7 = add nuw nsw i64 %2, 1
  %8 = trunc nuw nsw i64 %7 to i32
  %9 = uitofp nneg i32 %8 to double
  %10 = fmul double %9, 5.000000e-01
  %11 = getelementptr inbounds nuw i8, ptr %3, i64 8
  store double %10, ptr %11, align 8
  %12 = icmp eq i64 %7, 12
  br i1 %12, label %13, label %1

13:
  ret void
}
