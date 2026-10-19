define internal cc 1003 i32 @_both(i32 range(i32 0, 3) %0, i32 range(i32 0, 3) %1) memory(none) willreturn nounwind norecurse {
b1:
  %2 = icmp ne i32 %0, 0
  br i1 %2, label %b4, label %b3

b3:
  %3 = phi i32 [ 1, %b5 ], [ 0, %b4 ], [ 0, %b1 ]
  ret i32 %3

b4:
  %4 = icmp ne i32 %1, 0
  br i1 %4, label %b5, label %b3

b5:
  br label %b3
}

define internal cc 1003 i32 @_either(i32 range(i32 0, 3) %0, i32 range(i32 0, 3) %1) memory(none) willreturn nounwind norecurse {
b1:
  %2 = icmp ne i32 %0, 0
  br i1 %2, label %b4, label %b5

b3:
  %3 = phi i32 [ 1, %b4 ], [ 0, %b5 ]
  ret i32 %3

b4:
  br label %b3

b5:
  %4 = icmp ne i32 %1, 0
  br i1 %4, label %b4, label %b3
}

define i32 @f() {
b1:
  br label %b2

b2:
  %0 = phi i32 [ 0, %b1 ], [ %20, %b6 ]
  %1 = phi i32 [ 0, %b1 ], [ %18, %b6 ]
  %2 = phi i32 [ 0, %b1 ], [ %19, %b6 ]
  %3 = icmp samesign ult i32 %0, 3
  br i1 %3, label %b4, label %b3

b3:
  %4 = phi i32 [ %2, %b2 ]
  %5 = phi i32 [ %1, %b2 ]
  %6 = mul i32 %5, 7
  %7 = add i32 %6, %4
  ret i32 %7

b4:
  %8 = icmp ne i32 %0, 0
  br i1 %8, label %b13, label %b15

b6:
  %9 = phi i32 [ 0, %47 ], [ 1, %34 ]
  %10 = mul nsw i32 %45, 3
  %11 = add nsw i32 %10, %9
  %12 = add nsw i32 %11, 2
  %13 = call cc 1003 i32 @_both(i32 %0, i32 2)
  %14 = mul nsw i32 %13, 5
  %15 = add nsw i32 %12, %14
  %16 = call cc 1003 i32 @_either(i32 %0, i32 2)
  %17 = mul nsw i32 %16, 7
  %18 = add nsw i32 %15, %17
  %19 = add nsw i32 %46, 2
  %20 = add nuw nsw i32 %0, 1
  br label %b2

b13:
  %21 = phi i32 [ 0, %b15 ], [ 2, %b4 ]
  %22 = mul nsw i32 %1, 3
  %23 = add nsw i32 %22, %21
  %24 = call cc 1003 i32 @_both(i32 %0, i32 0)
  %25 = mul nsw i32 %24, 5
  %26 = add nsw i32 %23, %25
  %27 = call cc 1003 i32 @_either(i32 %0, i32 0)
  %28 = mul nsw i32 %27, 7
  %29 = add nsw i32 %26, %28
  br i1 %8, label %30, label %32

b15:
  br label %b13

30:
  %31 = add nsw i32 %2, 1
  br label %34

32:
  %33 = add nsw i32 %2, 0
  br label %34

34:
  %35 = phi i32 [ %31, %30 ], [ %33, %32 ]
  %36 = phi i32 [ 1, %30 ], [ 0, %32 ]
  %37 = mul nsw i32 %29, 3
  %38 = add nsw i32 %37, %36
  %39 = add nsw i32 %38, 2
  %40 = call cc 1003 i32 @_both(i32 %0, i32 1)
  %41 = mul nsw i32 %40, 5
  %42 = add nsw i32 %39, %41
  %43 = call cc 1003 i32 @_either(i32 %0, i32 1)
  %44 = mul nsw i32 %43, 7
  %45 = add nsw i32 %42, %44
  %46 = add nsw i32 %35, 2
  br i1 %8, label %b6, label %47

47:
  br label %b6
}
