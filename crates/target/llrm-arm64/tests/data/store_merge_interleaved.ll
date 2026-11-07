target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i16 @f(i16 %0) {
  %2 = alloca [8 x [8 x i16]], align 4
  %3 = alloca [8 x [8 x i16]], align 4
  %4 = alloca [8 x [8 x i64]], align 8



  %5 = add i16 %0, 1
  store i16 %5, ptr %2, align 4
  store i16 2, ptr %3, align 4
  %6 = add i16 %0, 2
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 2
  store i16 %6, ptr %7, align 2
  %8 = getelementptr inbounds nuw i8, ptr %3, i64 2
  store i16 1, ptr %8, align 2
  %9 = add i16 %0, 3
  %10 = getelementptr inbounds nuw i8, ptr %2, i64 4
  store i16 %9, ptr %10, align 4
  %11 = getelementptr inbounds nuw i8, ptr %3, i64 4
  store i16 2, ptr %11, align 4
  %12 = add i16 %0, 4
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 6
  store i16 %12, ptr %13, align 2
  %14 = getelementptr inbounds nuw i8, ptr %3, i64 6
  store i16 0, ptr %14, align 2
  %15 = add i16 %0, 5
  %16 = getelementptr inbounds nuw i8, ptr %2, i64 8
  store i16 %15, ptr %16, align 4
  %17 = getelementptr inbounds nuw i8, ptr %3, i64 8
  store i16 1, ptr %17, align 4
  %18 = add i16 %0, 6
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 10
  store i16 %18, ptr %19, align 2
  %20 = getelementptr inbounds nuw i8, ptr %3, i64 10
  store i16 2, ptr %20, align 2
  %21 = add i16 %0, 7
  %22 = getelementptr inbounds nuw i8, ptr %2, i64 12
  store i16 %21, ptr %22, align 4
  %23 = getelementptr inbounds nuw i8, ptr %3, i64 12
  store i16 0, ptr %23, align 4
  %24 = add i16 %0, 8
  %25 = getelementptr inbounds nuw i8, ptr %2, i64 14
  store i16 %24, ptr %25, align 2
  %26 = getelementptr inbounds nuw i8, ptr %3, i64 14
  store i16 1, ptr %26, align 2
  %27 = getelementptr inbounds nuw i8, ptr %2, i64 16
  %28 = getelementptr inbounds nuw i8, ptr %3, i64 16
  store i16 %12, ptr %27, align 4
  store i16 1, ptr %28, align 4
  %29 = getelementptr inbounds nuw i8, ptr %2, i64 18
  store i16 %15, ptr %29, align 2
  %30 = getelementptr inbounds nuw i8, ptr %3, i64 18
  store i16 2, ptr %30, align 2
  %31 = getelementptr inbounds nuw i8, ptr %2, i64 20
  store i16 %18, ptr %31, align 4
  %32 = getelementptr inbounds nuw i8, ptr %3, i64 20
  store i16 0, ptr %32, align 4
  %33 = getelementptr inbounds nuw i8, ptr %2, i64 22
  store i16 %21, ptr %33, align 2
  %34 = getelementptr inbounds nuw i8, ptr %3, i64 22
  store i16 1, ptr %34, align 2
  %35 = getelementptr inbounds nuw i8, ptr %2, i64 24
  store i16 %24, ptr %35, align 4
  %36 = getelementptr inbounds nuw i8, ptr %3, i64 24
  store i16 2, ptr %36, align 4
  %37 = add i16 %0, 9
  %38 = getelementptr inbounds nuw i8, ptr %2, i64 26
  store i16 %37, ptr %38, align 2
  %39 = getelementptr inbounds nuw i8, ptr %3, i64 26
  store i16 0, ptr %39, align 2
  %40 = add i16 %0, 10
  %41 = getelementptr inbounds nuw i8, ptr %2, i64 28
  store i16 %40, ptr %41, align 4
  %42 = getelementptr inbounds nuw i8, ptr %3, i64 28
  store i16 1, ptr %42, align 4
  %43 = add i16 %0, 11
  %44 = getelementptr inbounds nuw i8, ptr %2, i64 30
  store i16 %43, ptr %44, align 2
  %45 = getelementptr inbounds nuw i8, ptr %3, i64 30
  store i16 2, ptr %45, align 2
  %r = call i16 @use(ptr %2, ptr %3)
  ret i16 %r
}
declare i16 @use(ptr, ptr)
