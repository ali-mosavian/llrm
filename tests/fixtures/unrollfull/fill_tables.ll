@t6 = global [12 x i8] zeroinitializer
@t36 = global [12 x i8] zeroinitializer

define i16 @f() {
b0:
  br label %b23

b23:
  %i = phi i16 [ 0, %b0 ], [ %next, %b26 ]
  %go = icmp sle i16 %i, 5
  br i1 %go, label %b26, label %b27

b26:
  %m6 = mul i16 %i, 6
  %p6 = getelementptr inbounds i16, ptr @t6, i16 %i
  store i16 %m6, ptr %p6
  %m36 = mul i16 %i, 36
  %p36 = getelementptr inbounds i16, ptr @t36, i16 %i
  store i16 %m36, ptr %p36
  %next = add i16 %i, 1
  br label %b23

b27:
  %q = getelementptr inbounds i16, ptr @t6, i16 3
  %v = load i16, ptr %q
  ret i16 %v
}
