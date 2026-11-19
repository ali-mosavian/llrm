; hex.c through llrm-cc -O2, 2026-11-07 (blocks as clang numbers them)
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(i32 %0, ptr %1) {
  %3 = getelementptr inbounds nuw i8, ptr %1, i32 8
  store i8 0, ptr %3, align 1
  br label %4

4:
  %5 = phi i32 [ %17, %4 ], [ 0, %2 ]
  %6 = phi i32 [ %0, %2 ], [ %15, %4 ]
  %7 = and i32 %6, 15
  %8 = icmp samesign ult i32 %7, 10
  %9 = or disjoint i32 %7, 48
  %10 = add nuw nsw i32 %7, 87
  %11 = select i1 %8, i32 %9, i32 %10
  %12 = trunc nuw nsw i32 %11 to i8
  %13 = getelementptr i8, ptr %1, i32 %5
  %14 = getelementptr i8, ptr %13, i32 7
  store i8 %12, ptr %14, align 1
  %15 = lshr i32 %6, 4
  %16 = icmp eq i32 %15, 0
  %17 = add nsw i32 %5, -1
  br i1 %16, label %18, label %4

18:
  %19 = getelementptr i8, ptr %1, i32 %17
  %20 = getelementptr i8, ptr %19, i32 11
  %21 = load i8, ptr %20, align 1
  %22 = sext i8 %21 to i32
  %23 = add i32 %22, %11
  %24 = add i32 %17, %23
  %25 = add i32 %24, 8
  ret i32 %25
}
