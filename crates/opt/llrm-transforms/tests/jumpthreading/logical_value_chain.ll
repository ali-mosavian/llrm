define internal cc 1003 i32 @_both(i32 range(i32 0, 3) %0, i32 range(i32 0, 3) %1) memory(none) willreturn nounwind norecurse {
b1:
  %2 = icmp ne i32 %0, 0
  br i1 %2, label %b4, label %b2

b2:
  br label %b3

b3:
  %3 = phi i32 [ 1, %b5 ], [ 0, %b2 ]
  ret i32 %3

b4:
  %4 = icmp ne i32 %1, 0
  br i1 %4, label %b5, label %b2

b5:
  br label %b3
}

define internal cc 1003 i32 @_either(i32 range(i32 0, 3) %0, i32 range(i32 0, 3) %1) memory(none) willreturn nounwind norecurse {
b1:
  %2 = icmp ne i32 %0, 0
  br i1 %2, label %b4, label %b5

b2:
  br label %b3

b3:
  %3 = phi i32 [ 1, %b4 ], [ 0, %b2 ]
  ret i32 %3

b4:
  br label %b3

b5:
  %4 = icmp ne i32 %1, 0
  br i1 %4, label %b4, label %b2
}

define i32 @f() {
b1:
  br label %b2

b2:
  %0 = phi i32 [ 0, %b1 ], [ %9, %b6 ]
  %1 = phi i32 [ 0, %b1 ], [ %54, %b6 ]
  %2 = phi i32 [ 0, %b1 ], [ %58, %b6 ]
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
  br i1 %8, label %b10, label %b15

b6:
  %9 = add nuw nsw i32 %0, 1
  br label %b2

b10:
  br label %b13

b13:
  %10 = phi i32 [ 0, %b15 ], [ 2, %b10 ]
  %11 = mul nsw i32 %1, 3
  %12 = add nsw i32 %11, %10
  %13 = call cc 1003 i32 @_both(i32 %0, i32 0)
  %14 = mul nsw i32 %13, 5
  %15 = add nsw i32 %12, %14
  %16 = call cc 1003 i32 @_either(i32 %0, i32 0)
  %17 = mul nsw i32 %16, 7
  %18 = add nsw i32 %15, %17
  br i1 %8, label %b19, label %b18

b15:
  br label %b13

b18:
  br label %b24

b19:
  br label %b22

b22:
  %19 = phi i32 [ 0, %b24 ], [ 1, %b19 ]
  %20 = add nsw i32 %2, %19
  br i1 %8, label %21, label %23

b24:
  br label %b22

21:
  br label %22

22:
  br label %25

23:
  br label %24

24:
  br label %25

25:
  %26 = phi i32 [ 1, %22 ], [ 0, %24 ]
  %27 = mul nsw i32 %18, 3
  %28 = add nsw i32 %27, %26
  %29 = add nsw i32 %28, 2
  %30 = call cc 1003 i32 @_both(i32 %0, i32 1)
  %31 = mul nsw i32 %30, 5
  %32 = add nsw i32 %29, %31
  %33 = call cc 1003 i32 @_either(i32 %0, i32 1)
  %34 = mul nsw i32 %33, 7
  %35 = add nsw i32 %32, %34
  br i1 %8, label %36, label %37

36:
  br label %37

37:
  br label %38

38:
  %39 = add nsw i32 %20, 2
  br i1 %8, label %40, label %42

40:
  br label %41

41:
  br label %44

42:
  br label %43

43:
  br label %44

44:
  %45 = phi i32 [ 1, %41 ], [ 0, %43 ]
  %46 = mul nsw i32 %35, 3
  %47 = add nsw i32 %46, %45
  %48 = add nsw i32 %47, 2
  %49 = call cc 1003 i32 @_both(i32 %0, i32 2)
  %50 = mul nsw i32 %49, 5
  %51 = add nsw i32 %48, %50
  %52 = call cc 1003 i32 @_either(i32 %0, i32 2)
  %53 = mul nsw i32 %52, 7
  %54 = add nsw i32 %51, %53
  br i1 %8, label %55, label %56

55:
  br label %56

56:
  br label %57

57:
  %58 = add nsw i32 %39, 2
  br label %b6
}
