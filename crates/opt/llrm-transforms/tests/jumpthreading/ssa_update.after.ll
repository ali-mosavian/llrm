define i32 @f(i1 %x, i1 %y, i32 %a) {
entry:
  br i1 %x, label %join.thread, label %join

join.thread:
  call void @sink(i32 1)
  %v1 = add i32 %a, 7
  br label %t

join:
  call void @sink(i32 2)
  %v = add i32 %a, 7
  br i1 %y, label %t, label %e

t:
  %v2 = phi i32 [ %v1, %join.thread ], [ %v, %join ]
  %r = mul i32 %v2, 2
  ret i32 %r

e:
  ret i32 0
}

declare void @sink(i32)
