define i32 @f(ptr %p, i32 %n) {
entry:
  %f = load i32, ptr %p
  br label %loop
loop:
  %i = phi i32 [1, %entry], [%i1, %loop]
  %m = phi i32 [%f, %entry], [%m1, %loop]
  %q = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %q
  %c = icmp sgt i32 %v, %m
  %m1 = select i1 %c, i32 %v, i32 %m
  %i1 = add i32 %i, 1
  %d = icmp slt i32 %i1, %n
  br i1 %d, label %loop, label %exit
exit:
  ret i32 %m1
}
