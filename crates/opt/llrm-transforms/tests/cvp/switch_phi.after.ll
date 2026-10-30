define range(i32 0, 4) i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 3
  br i1 %c, label %sw, label %out

sw:
  %cond = icmp eq i32 %x, 0
  br i1 %cond, label %a, label %m

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
