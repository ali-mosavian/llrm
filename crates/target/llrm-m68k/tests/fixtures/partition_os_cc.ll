; partition.c through llrm-cc -Os, 2026-11-07, the IR LSR reads (blocks as clang numbers them)
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1) optsize {
  %3 = add nsw i32 %1, -1
  %4 = getelementptr inbounds [4 x i8], ptr %0, i32 %3
  %5 = load i32, ptr %4, align 2
  %6 = icmp sgt i32 %1, 1
  br i1 %6, label %7, label %10

7:
  br label %18

8:
  %9 = load i32, ptr %4, align 2
  br label %10

10:
  %11 = phi i32 [ %5, %2 ], [ %9, %8 ]
  %12 = phi i32 [ 0, %2 ], [ %29, %8 ]
  %13 = getelementptr inbounds [4 x i8], ptr %0, i32 %12
  %14 = load i32, ptr %13, align 2
  store i32 %11, ptr %13, align 2
  store i32 %14, ptr %4, align 2
  %15 = load i32, ptr %0, align 2
  %16 = shl nsw i32 %15, 4
  %17 = add nsw i32 %16, %12
  ret i32 %17

18:
  %19 = phi i32 [ %29, %28 ], [ 0, %7 ]
  %20 = phi i32 [ %30, %28 ], [ 0, %7 ]
  %21 = getelementptr inbounds nuw [4 x i8], ptr %0, i32 %20
  %22 = load i32, ptr %21, align 2
  %23 = icmp slt i32 %22, %5
  br i1 %23, label %24, label %28

24:
  %25 = getelementptr inbounds [4 x i8], ptr %0, i32 %19
  %26 = load i32, ptr %25, align 2
  store i32 %22, ptr %25, align 2
  store i32 %26, ptr %21, align 2
  %27 = add nsw i32 %19, 1
  br label %28

28:
  %29 = phi i32 [ %27, %24 ], [ %19, %18 ]
  %30 = add nuw nsw i32 %20, 1
  %31 = icmp slt i32 %30, %3
  br i1 %31, label %18, label %8
}
