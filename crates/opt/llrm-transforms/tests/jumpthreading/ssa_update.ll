; updateSSA: %v is defined in the threaded block and read below it, so %t, reached by the copy and the original, needs a phi.
define i32 @f(i1 %x, i1 %y, i32 %a) {
entry:
  br i1 %x, label %p1, label %p2

p1:
  call void @sink(i32 1)
  br label %join

p2:
  call void @sink(i32 2)
  br label %join

join:
  %c = phi i1 [ true, %p1 ], [ %y, %p2 ]
  %v = add i32 %a, 7
  br i1 %c, label %t, label %e

t:
  %r = mul i32 %v, 2
  ret i32 %r

e:
  ret i32 0
}

declare void @sink(i32)
