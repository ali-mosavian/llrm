; tryToUnfoldSelect(CmpInst): the select in %p1 has an arm (0) that decides the compare in %bb and one (%a) that does not, so it becomes a branch.
define i32 @f(i1 %k, i1 %s, i32 %a) {
entry:
  br i1 %k, label %p1, label %p2

p1:
  %sel = select i1 %s, i32 0, i32 %a
  br label %bb

p2:
  br label %bb

bb:
  %v = phi i32 [ %sel, %p1 ], [ %a, %p2 ]
  %cmp = icmp eq i32 %v, 0
  br i1 %cmp, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}
