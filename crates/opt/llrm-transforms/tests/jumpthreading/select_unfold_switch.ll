; tryToUnfoldSelect(SwitchInst): the select in %p1 feeds the phi %v that %bb switches on, so it becomes a branch.
define i32 @f(i1 %k, i1 %s, i32 %a) {
entry:
  br i1 %k, label %p1, label %p2

p1:
  %sel = select i1 %s, i32 1, i32 %a
  br label %bb

p2:
  br label %bb

bb:
  %v = phi i32 [ %sel, %p1 ], [ %a, %p2 ]
  switch i32 %v, label %d [
    i32 1, label %t
  ]

t:
  ret i32 1

d:
  ret i32 2
}
