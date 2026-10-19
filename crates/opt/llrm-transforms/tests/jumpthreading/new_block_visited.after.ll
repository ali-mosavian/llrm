declare void @use(i16)

declare void @side()

define void @f(i1 %c, i1 %d) {
b50:
  br i1 %c, label %b59, label %b53

b53:
  %t = trunc i32 1 to i16
  br label %b58

b58:
  %t2 = phi i16 [ %t3, %b60 ], [ %t3, %b59 ], [ %t, %b53 ]
  call void @use(i16 %t2)
  ret void

b59:
  %t3 = phi i16 [ 2, %b50 ]
  br i1 %d, label %b60, label %b58

b60:
  call void @side()
  br label %b58
}
