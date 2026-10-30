; removePredecessor: the phi loses the edge of the case that went.
define i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 3
  br i1 %c, label %sw, label %out

sw:
  switch i32 %x, label %m [
    i32 0, label %a
    i32 9, label %b
  ]

a:
  br label %m

b:
  br label %m

m:
  %p = phi i32 [ 1, %sw ], [ 2, %a ], [ 3, %b ]
  ret i32 %p

out:
  ret i32 0
}
