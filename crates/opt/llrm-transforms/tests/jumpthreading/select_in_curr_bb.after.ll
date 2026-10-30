define i32 @f(i1 %k, i32 %x, i32 %y) {
entry:
  br i1 %k, label %1, label %0

0:
  br label %1

1:
  %2 = phi i32 [ %x, %0 ], [ %y, %entry ]
  ret i32 %2
}
