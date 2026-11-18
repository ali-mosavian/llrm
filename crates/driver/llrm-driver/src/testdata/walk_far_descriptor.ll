target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

define cc 1000 i32 @"WALK&"(i16 %0, i16 %1, i16 %2) optsize nounwind {
b1:
  %3 = icmp eq i16 %1, 0
  br i1 %3, label %b5, label %4

4:
  %5 = inttoptr i16 %0 to ptr addrspace(2)
  br label %b3

b3:
  %6 = phi i16 [ %2, %4 ], [ %14, %b3 ]
  %7 = phi i16 [ %1, %4 ], [ %21, %b3 ]
  %8 = phi i32 [ 0, %4 ], [ %23, %b3 ]
  %9 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %10 = mul i16 %7, 6
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 %10
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %13 = load i16, ptr addrspace(1) %12
  %14 = add i16 %6, 1
  %15 = tail call cc 1000 i32 @"WALK&"(i16 %0, i16 %13, i16 %14)
  %16 = load i16, ptr addrspace(1) %11
  %17 = sext i16 %6 to i32
  %18 = sext i16 %16 to i32
  %19 = mul nsw i32 %18, %17
  %20 = getelementptr i8, ptr addrspace(1) %11, i16 4
  %21 = load i16, ptr addrspace(1) %20
  %22 = add i32 %15, %8
  %23 = add i32 %22, %19
  %24 = icmp eq i16 %21, 0
  br i1 %24, label %b5, label %b3

b5:
  %accumulator.tr.lcssa = phi i32 [ 0, %b1 ], [ %23, %b3 ]
  ret i32 %accumulator.tr.lcssa
}
