; duplicateCondBranchOnPHIIntoPred (by processBranchOnPHI): the branch on a phi of two compares is copied into each predecessor, where it branches on the compare.
define i32 @f(float %x, float %y, i1 %k) {
entry:
  br i1 %k, label %p1, label %p2

p1:
  %a = fcmp oge float %x, %y
  br label %j

p2:
  %b = fcmp ole float %x, %y
  br label %j

j:
  %ph = phi i1 [ %a, %p1 ], [ %b, %p2 ]
  br i1 %ph, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}
