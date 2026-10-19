define i32 @f(float %x, float %y, i1 %k) {
entry:
  br i1 %k, label %p1, label %j

p1:
  %a = fcmp oge float %x, %y
  br i1 %a, label %t, label %e

j:
  %b = fcmp ole float %x, %y
  br i1 %b, label %t, label %e

t:
  ret i32 1

e:
  ret i32 2
}
