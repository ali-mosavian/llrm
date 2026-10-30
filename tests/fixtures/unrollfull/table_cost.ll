@w = constant [10 x i16] [i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0, i16 0]

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
  %t0 = mul i16 %wv, %xv
  %u0 = add i16 %t0, 1
  %t1 = mul i16 %wv, %u0
  %u1 = add i16 %t1, 2
  %t2 = mul i16 %wv, %u1
  %u2 = add i16 %t2, 3
  %t3 = mul i16 %wv, %u2
  %u3 = add i16 %t3, 4
  %t4 = mul i16 %wv, %u3
  %u4 = add i16 %t4, 5
  %t5 = mul i16 %wv, %u4
  %u5 = add i16 %t5, 6
  %t6 = mul i16 %wv, %u5
  %u6 = add i16 %t6, 7
  %t7 = mul i16 %wv, %u6
  %u7 = add i16 %t7, 8
  %t8 = mul i16 %wv, %u7
  %u8 = add i16 %t8, 9
  %t9 = mul i16 %wv, %u8
  %u9 = add i16 %t9, 10
  %t10 = mul i16 %wv, %u9
  %u10 = add i16 %t10, 11
  %t11 = mul i16 %wv, %u10
  %u11 = add i16 %t11, 12
  %s = add i16 %acc, %u11
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}
