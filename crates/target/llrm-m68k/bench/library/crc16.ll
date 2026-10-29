target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1) {
  %3 = icmp eq i32 %1, 0
  br i1 %3, label %55, label %4

4:                                                ; preds = %2, %4
  %5 = phi i32 [ %8, %4 ], [ %1, %2 ]
  %6 = phi i32 [ %53, %4 ], [ 65535, %2 ]
  %7 = phi ptr [ %52, %4 ], [ %0, %2 ]
  %8 = add i32 %5, -1
  %9 = load i8, ptr %7, align 1
  %10 = zext i8 %9 to i32
  %11 = shl i32 %10, 8
  %12 = xor i32 %11, %6
  %13 = icmp ult i32 %12, 32768
  %14 = shl i32 %12, 1
  %15 = xor i32 %14, 4129
  %16 = select i1 %13, i32 %14, i32 %15
  %17 = and i32 %16, 32768
  %18 = icmp eq i32 %17, 0
  %19 = shl i32 %16, 1
  %20 = xor i32 %19, 4129
  %21 = select i1 %18, i32 %19, i32 %20
  %22 = and i32 %21, 32768
  %23 = icmp eq i32 %22, 0
  %24 = shl i32 %21, 1
  %25 = xor i32 %24, 4129
  %26 = select i1 %23, i32 %24, i32 %25
  %27 = and i32 %26, 32768
  %28 = icmp eq i32 %27, 0
  %29 = shl i32 %26, 1
  %30 = xor i32 %29, 4129
  %31 = select i1 %28, i32 %29, i32 %30
  %32 = and i32 %31, 32768
  %33 = icmp eq i32 %32, 0
  %34 = shl i32 %31, 1
  %35 = xor i32 %34, 4129
  %36 = select i1 %33, i32 %34, i32 %35
  %37 = and i32 %36, 32768
  %38 = icmp eq i32 %37, 0
  %39 = shl i32 %36, 1
  %40 = xor i32 %39, 4129
  %41 = select i1 %38, i32 %39, i32 %40
  %42 = and i32 %41, 32768
  %43 = icmp eq i32 %42, 0
  %44 = shl i32 %41, 1
  %45 = xor i32 %44, 4129
  %46 = select i1 %43, i32 %44, i32 %45
  %47 = and i32 %46, 32768
  %48 = icmp eq i32 %47, 0
  %49 = shl i32 %46, 1
  %50 = xor i32 %49, 4129
  %51 = select i1 %48, i32 %49, i32 %50
  %52 = getelementptr i8, ptr %7, i32 1
  %53 = and i32 %51, 65535
  %54 = icmp eq i32 %8, 0
  br i1 %54, label %55, label %4

55:                                               ; preds = %4, %2
  %56 = phi i32 [ 65535, %2 ], [ %53, %4 ]
  %57 = lshr i32 %56, 8
  %58 = and i32 %56, 255
  %59 = xor i32 %57, %58
  ret i32 %59
}
