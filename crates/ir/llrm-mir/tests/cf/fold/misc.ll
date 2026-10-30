%P = type { i32, i1 }
@g = constant i32 5
define i32 @m(i1 %c) {
entry:
  br i1 %c, label %a, label %b
a:
  br label %j
b:
  br label %j
j:
  %p1 = phi i32 [ 5, %a ], [ 5, %b ]
  %p2 = phi i32 [ 5, %a ], [ 6, %b ]
  %p3 = phi i32 [ poison, %a ], [ 7, %b ]
  %p4 = phi i32 [ poison, %a ], [ poison, %b ]
  %s1 = select i1 true, i32 1, i32 2
  %s2 = select i1 false, i32 1, i32 2
  %s3 = select i1 poison, i32 1, i32 2
  %f1 = freeze i32 5
  %f2 = freeze i32 poison
  %f3 = freeze ptr @g
  %e1 = extractvalue %P { i32 3, i1 true }, 0
  %e2 = extractvalue %P { i32 3, i1 true }, 1
  %e3 = extractvalue %P zeroinitializer, 0
  %i1 = insertvalue %P { i32 3, i1 true }, i32 9, 0
  %i2 = insertvalue %P zeroinitializer, i1 true, 1
  %i3 = insertvalue [2 x i32] zeroinitializer, i32 0, 1
  %a1 = add i32 1, 2
  %a2 = add i32 poison, 2
  %a3 = udiv i32 1, 0
  %a4 = sdiv i32 -2147483648, -1
  %a5 = shl i32 1, 33
  %a6 = and i32 ptrtoint (ptr @g to i32), 3
  %a7 = sub i64 ptrtoint (ptr getelementptr inbounds (i8, ptr @g, i64 3) to i64), ptrtoint (ptr @g to i64)
  %a8 = add i128 340282366920938463463374607431768211455, 1
  %a9 = mul i7 100, 3
  %b1 = lshr i64 -1, 63
  %b2 = ashr i64 -1, 63
  %b3 = xor i64 ptrtoint (ptr @g to i64), 0
  %b4 = or i32 ptrtoint (ptr @g to i32), -1
  %b5 = trunc i32 300 to i8
  %b6 = icmp sgt i128 -1, 0
  %l = alloca i32
  ret i32 %p1
}
