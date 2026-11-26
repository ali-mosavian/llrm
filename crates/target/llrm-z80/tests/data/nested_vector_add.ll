target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

@px = global [101 x i16] zeroinitializer, align 2
@vx = global [101 x i16] zeroinitializer, align 2
@py = global [101 x i16] zeroinitializer, align 2
@vy = global [101 x i16] zeroinitializer, align 2
@pz = global [101 x i16] zeroinitializer, align 2
@vz = global [101 x i16] zeroinitializer, align 2

define void @run() {
  br label %1

1:                                                ; preds = %0, %22
  %2 = phi i16 [ 1, %0 ], [ %23, %22 ]
  br label %3

3:                                                ; preds = %1, %3
  %4 = phi i16 [ 0, %1 ], [ %20, %3 ]
  %5 = getelementptr inbounds [2 x i8], ptr @px, i16 %4
  %6 = load i16, ptr %5, align 2
  %7 = getelementptr inbounds [2 x i8], ptr @vx, i16 %4
  %8 = load i16, ptr %7, align 2
  %9 = add i16 %8, %6
  store i16 %9, ptr %5, align 2
  %10 = getelementptr inbounds [2 x i8], ptr @py, i16 %4
  %11 = load i16, ptr %10, align 2
  %12 = getelementptr inbounds [2 x i8], ptr @vy, i16 %4
  %13 = load i16, ptr %12, align 2
  %14 = add i16 %13, %11
  store i16 %14, ptr %10, align 2
  %15 = getelementptr inbounds [2 x i8], ptr @pz, i16 %4
  %16 = load i16, ptr %15, align 2
  %17 = getelementptr inbounds [2 x i8], ptr @vz, i16 %4
  %18 = load i16, ptr %17, align 2
  %19 = add i16 %18, %16
  store i16 %19, ptr %15, align 2
  %20 = add i16 %4, 1
  %21 = icmp eq i16 %20, 101
  br i1 %21, label %22, label %3

22:                                               ; preds = %3
  %23 = add i16 %2, 1
  %24 = icmp eq i16 %23, 101
  br i1 %24, label %25, label %1

25:                                               ; preds = %22
  ret void
}



