declare void @ebnd()
define i32 @dot(ptr addrspace(1) %0, ptr addrspace(1) %1) {
b1:
  %2 = load i16, ptr addrspace(1) %0
  %3 = load i16, ptr addrspace(1) %1
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %5 = load ptr addrspace(1), ptr addrspace(1) %4
  %6 = getelementptr i8, ptr addrspace(1) %1, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  br label %b3
b3:
  %8 = phi i16 [ 0, %b1 ], [ %25, %b11 ]
  %9 = phi i32 [ 0, %b1 ], [ %24, %b11 ]
  %10 = icmp ult i16 %8, %2
  br i1 %10, label %b6, label %b7
b4:
  %exitcond = icmp ne i16 %8, %2
  br i1 %exitcond, label %b9, label %b10
b5:
  %11 = phi i32 [ %9, %b7 ]
  ret i32 %11
b6:
  %12 = icmp ult i16 %8, %3
  %13 = zext i1 %12 to i8
  br label %b7
b7:
  %14 = phi i8 [ %13, %b6 ], [ 0, %b3 ]
  %15 = icmp ne i8 %14, 0
  br i1 %15, label %b4, label %b5
b9:
  %16 = mul nuw i16 %8, 2
  %17 = getelementptr inbounds i8, ptr addrspace(1) %5, i16 %16
  %exitcond1 = icmp ne i16 %8, %3
  br i1 %exitcond1, label %b11, label %b12
b10:
  call void @ebnd()
  unreachable
b11:
  %18 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %16
  %19 = load i16, ptr addrspace(1) %17
  %20 = sext i16 %19 to i32
  %21 = load i16, ptr addrspace(1) %18
  %22 = sext i16 %21 to i32
  %23 = mul nsw i32 %20, %22
  %24 = add i32 %9, %23
  %25 = add nuw i16 %8, 1
  br label %b3
b12:
  call void @ebnd()
  unreachable
}
