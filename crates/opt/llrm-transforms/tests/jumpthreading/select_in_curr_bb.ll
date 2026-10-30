; tryToUnfoldSelectInCurrBB: a select on a phi with constant inputs becomes a branch (the select reads %p, a phi of false and true).
define i32 @f(i1 %k, i32 %x, i32 %y) {
entry:
  br i1 %k, label %b1, label %b2

b1:
  br label %bb

b2:
  br label %bb

bb:
  %p = phi i1 [ false, %b1 ], [ true, %b2 ]
  %s = select i1 %p, i32 %x, i32 %y
  ret i32 %s
}
