declare void @use(i16)
declare void @side()

define void @f(i1 %c, i1 %d) {
entry:
  br label %b50
b50:
  br i1 %c, label %b54, label %b52
b52:
  br label %b56
b53:
  %p = phi i32 [ 2, %b54 ], [ 1, %b56 ]
  %t = trunc i32 %p to i16
  br i1 %c, label %b59, label %b58
b54:
  br label %b53
b56:
  br label %b53
b58:
  call void @use(i16 %t)
  ret void
b59:
  br i1 %d, label %b60, label %b58
b60:
  call void @side()
  br label %b58
}
