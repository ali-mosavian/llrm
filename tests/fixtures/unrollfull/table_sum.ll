@w = constant [8 x i16] [i16 0, i16 3, i16 0, i16 0, i16 5, i16 0, i16 0, i16 7]

define i16 @f(ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ 0, %b0 ], [ %s, %b1 ]
  %wp = getelementptr i16, ptr @w, i16 %i
  %wv = load i16, ptr %wp
  %xp = getelementptr i16, ptr %p, i16 %i
  %xv = load i16, ptr %xp
  %m = mul i16 %wv, %xv
  %s = add i16 %acc, %m
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 8
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}
