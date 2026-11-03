target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
%union.D = type { [2 x i32] }
@NV = external dso_local global i16, align 2
@V = external dso_local constant [0 x [12 x i32]], align 2
@bad = internal global [4 x [4 x i16]] zeroinitializer, align 2
define dso_local i16 @main() {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca ptr, align 2
  %4 = alloca %union.D, align 1
  %5 = alloca %union.D, align 1
  %6 = alloca %union.D, align 1
  store i16 0, ptr %2, align 2
  br label %7
  %8 = load i16, ptr %2, align 2
  %9 = load i16, ptr @NV, align 2
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %12, label %11
  br label %37
  %13 = load i16, ptr %2, align 2
  %14 = getelementptr inbounds [0 x [12 x i32]], ptr @V, i16 0, i16 %13
  %15 = getelementptr inbounds [12 x i32], ptr %14, i16 0, i16 0
  store ptr %15, ptr %3, align 2
  %16 = load i64, ptr %4, align 1
  %17 = load i64, ptr %5, align 1
  %18 = call i64 @f2(i64 noundef %16, i64 noundef %17)
  store i64 %18, ptr %6, align 1
  %19 = load ptr, ptr %3, align 2
  %20 = getelementptr inbounds i32, ptr %19, i16 8
  %21 = load i32, ptr %20, align 2
  %22 = load ptr, ptr %3, align 2
  %23 = getelementptr inbounds i32, ptr %22, i16 9
  %24 = load i32, ptr %23, align 2
  call void @chk(i16 noundef 2, ptr noundef byval(%union.D) align 1 %6, i32 noundef %21, i32 noundef %24)
  %25 = load i64, ptr %4, align 1
  %26 = load i64, ptr %5, align 1
  %27 = call i64 @f3(i64 noundef %25, i64 noundef %26)
  %28 = load ptr, ptr %3, align 2
  %29 = getelementptr inbounds i32, ptr %28, i16 10
  %30 = load i32, ptr %29, align 2
  %31 = load ptr, ptr %3, align 2
  %32 = getelementptr inbounds i32, ptr %31, i16 11
  %33 = load i32, ptr %32, align 2
  br label %34
  %35 = load i16, ptr %2, align 2
  %36 = add nsw i16 %35, 1
  br label %7
  %38 = load i16, ptr %1, align 2
  ret i16 %38
}
declare dso_local i64 @f2(i64 noundef, i64 noundef)
define internal void @chk(i16 noundef %0, ptr noundef byval(%union.D) align 1 %1, i32 noundef %2, i32 noundef %3) {
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  %10 = alloca i32, align 2
  store i32 %2, ptr %6, align 2
  store i32 %3, ptr %7, align 2
  %11 = load i32, ptr %6, align 2
  %12 = lshr i32 %11, 20
  %13 = and i32 %12, 2047
  %14 = load i32, ptr %6, align 2
  %15 = shl i32 %14, 1
  %16 = load i32, ptr %7, align 2
  %17 = or i32 %15, %16
  %18 = icmp eq i32 %17, 0
  br i1 %18, label %19, label %20
  br label %31
  %21 = load i32, ptr %8, align 2
  %22 = icmp eq i32 %21, 0
  br i1 %22, label %23, label %24
  br label %29
  %25 = load i32, ptr %8, align 2
  %26 = icmp eq i32 %25, 2047
  %27 = zext i1 %26 to i64
  %28 = select i1 %26, i16 2, i16 3
  br label %29
  %30 = phi i16 [ 1, %23 ], [ %28, %24 ]
  br label %31
  %32 = phi i16 [ 0, %19 ], [ %30, %29 ]
  %33 = getelementptr inbounds [2 x i32], ptr %1, i16 0, i16 1
  %34 = load i32, ptr %33, align 1
  %35 = load i32, ptr %6, align 2
  %36 = icmp eq i32 %34, %35
  br i1 %36, label %37, label %42
  %38 = getelementptr inbounds [2 x i32], ptr %1, i16 0, i16 0
  %39 = load i32, ptr %38, align 1
  %40 = load i32, ptr %7, align 2
  %41 = icmp eq i32 %39, %40
  br i1 %41, label %54, label %42
  %43 = getelementptr inbounds [2 x i32], ptr %1, i16 0, i16 1
  %44 = load i32, ptr %43, align 1
  %45 = getelementptr inbounds [2 x i32], ptr %1, i16 0, i16 0
  %46 = load i32, ptr %45, align 1
  %47 = call i16 @nan64(i32 noundef %44, i32 noundef %46)
  %48 = icmp ne i16 %47, 0
  br i1 %48, label %49, label %55
  %50 = load i32, ptr %6, align 2
  %51 = load i32, ptr %7, align 2
  %52 = call i16 @nan64(i32 noundef %50, i32 noundef %51)
  %53 = icmp ne i16 %52, 0
  br i1 %53, label %54, label %55
  br label %62
  %56 = load i16, ptr %5, align 2
  %57 = getelementptr inbounds [4 x [4 x i16]], ptr @bad, i16 0, i16 %56
  %58 = load i16, ptr %9, align 2
  %59 = getelementptr inbounds [4 x i16], ptr %57, i16 0, i16 %58
  %60 = load i16, ptr %59, align 2
  %61 = add i16 %60, 1
  store i16 %61, ptr %59, align 2
  br label %62
  %63 = load i32, ptr %10, align 2
  switch i32 %63, label %65 [
  ]
  ret void
  unreachable
}
declare dso_local i64 @f3(i64 noundef, i64 noundef)
define internal i16 @nan64(i32 noundef %0, i32 noundef %1) {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = load i32, ptr %3, align 2
  %6 = and i32 %5, 2146435072
  %7 = icmp eq i32 %6, 2146435072
  br i1 %7, label %8, label %14
  %9 = load i32, ptr %3, align 2
  %10 = and i32 %9, 1048575
  %11 = load i32, ptr %4, align 2
  %12 = or i32 %10, %11
  %13 = icmp ne i32 %12, 0
  br label %14
  %15 = phi i1 [ false, %2 ], [ %13, %8 ]
  %16 = zext i1 %15 to i16
  ret i16 %16
}
