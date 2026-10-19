define i32 @f(i1 %k, i1 %s, i32 %a) {
entry:
  br i1 %k, label %p1, label %bb

p1:
  br i1 %s, label %t, label %bb

bb:
  %v = phi i32 [ %a, %p1 ], [ %a, %entry ]
  switch i32 %v, label %d [
    i32 1, label %t
  ]

t:
  ret i32 1

d:
  ret i32 2
}
