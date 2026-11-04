target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(i32 %0, ptr initializes((8, 9)) %1) memory(argmem: readwrite) nounwind nofree nosync norecurse {
  %3 = getelementptr i8, ptr %1, i32 8
  store i8 0, ptr %3, align 1
  %4 = getelementptr i8, ptr %1, i32 11
  br label %5

5:
  %6 = phi i32 [ %19, %5 ], [ 8, %2 ]
  %7 = phi ptr [ %18, %5 ], [ %4, %2 ]
  %8 = phi i32 [ %0, %2 ], [ %16, %5 ]
  %9 = and i32 %8, 15
  %10 = icmp samesign ult i32 %9, 10
  %11 = or disjoint i32 %9, 48
  %12 = add nuw nsw i32 %9, 87
  %13 = select i1 %10, i32 %11, i32 %12
  %14 = trunc nuw nsw i32 %13 to i8
  %15 = getelementptr i8, ptr %7, i32 -4
  store i8 %14, ptr %15, align 1
  %16 = lshr i32 %8, 4
  %17 = icmp ult i32 %8, 16
  %18 = getelementptr i8, ptr %7, i32 -1
  %19 = add nsw i32 %6, -1
  br i1 %17, label %20, label %5

20:
  %21 = phi i32 [ %19, %5 ]
  %22 = phi i32 [ %13, %5 ]
  %23 = phi ptr [ %18, %5 ]
  %24 = load i8, ptr %23, align 1
  %25 = sext i8 %24 to i32
  %26 = add i32 %25, %22
  %27 = add i32 %21, %26
  ret i32 %27
}
