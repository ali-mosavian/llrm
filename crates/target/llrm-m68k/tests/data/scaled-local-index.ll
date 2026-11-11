target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@zero = external constant i8, align 1
@buf = external constant [16 x i8], align 1

define i16 @test() {
  %1 = alloca [4 x i16], align 2
  %2 = load volatile i8, ptr @zero, align 1
  %3 = load volatile i8, ptr @zero, align 1
  br label %4

4:                                                ; preds = %0, %4
  %5 = phi i8 [ 0, %0 ], [ %7, %4 ]
  %6 = load volatile i8, ptr @zero, align 1
  %7 = add i8 %5, 1
  %8 = icmp eq i8 %7, 8
  br i1 %8, label %9, label %4

9:                                                ; preds = %4
  %10 = load volatile i8, ptr @zero, align 1
  %11 = load volatile i8, ptr @zero, align 1
  %12 = load volatile i8, ptr @zero, align 1
  %13 = load volatile i8, ptr @zero, align 1
  %14 = load volatile i8, ptr @zero, align 1
  %15 = load volatile i8, ptr @zero, align 1
  br label %16

16:                                               ; preds = %16, %9
  %17 = phi i32 [ 0, %9 ], [ %25, %16 ]
  %18 = mul i32 %17, 7
  %19 = add i32 %18, 3
  %20 = load volatile i8, ptr @zero, align 1
  %21 = zext i8 %20 to i32
  %22 = add i32 %19, %21
  %23 = trunc i32 %22 to i16
  %24 = getelementptr inbounds [2 x i8], ptr %1, i32 %17
  store i16 %23, ptr %24, align 2
  %25 = add i32 %17, 1
  %26 = icmp eq i32 %25, 4
  br i1 %26, label %27, label %16

27:                                               ; preds = %16
  %28 = zext i8 %3 to i16
  %29 = add i16 %28, 504
  %30 = add i8 %15, -27
  %31 = add i8 %14, -1
  %32 = and i8 %31, 3
  %33 = zext nneg i8 %32 to i32
  %34 = getelementptr inbounds [2 x i8], ptr %1, i32 %33
  %35 = load i16, ptr %34, align 2
  %36 = and i16 %35, 255
  %37 = load i8, ptr @buf, align 1
  %38 = zext i8 %37 to i16
  %39 = icmp ugt i16 %36, %38
  %40 = getelementptr inbounds i8, ptr %1, i32 2
  %41 = load i16, ptr %40, align 2
  %42 = load i16, ptr %1, align 2
  %43 = select i1 %39, i16 %41, i16 %42
  %44 = icmp ugt i16 %43, 940
  %45 = and i8 %31, 1
  %46 = icmp eq i8 %45, 0
  %47 = or i1 %46, %44
  %48 = shl i8 %30, 1
  %49 = xor i8 %48, 8
  %50 = select i1 %47, i8 %49, i8 %30
  %51 = add i8 %13, 32
  %52 = trunc i16 %41 to i8
  %53 = or i8 %51, %52
  %54 = add i8 %11, -34
  %55 = icmp eq i8 %13, %54
  %56 = select i1 %55, i8 0, i8 %53
  %57 = xor i8 %50, %56
  %58 = zext i8 %57 to i16
  %59 = shl i16 %58, 1
  %60 = xor i16 %59, %29
  ret i16 %60
}





