define i32 @f(i1 %k, i1 %s, i32 %a) {
entry:
  br i1 %k, label %p1, label %bb

p1:
  br i1 %s, label %t, label %bb

bb:
  %v = phi i32 [ %a, %p1 ], [ %a, %entry ]
  %cmp = icmp eq i32 %v, 0
  br i1 %cmp, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}
