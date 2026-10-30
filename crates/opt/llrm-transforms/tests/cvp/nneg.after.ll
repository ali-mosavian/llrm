define float @f(i16 %x, i16 %y, i16 %z) {
entry:
  %a = and i16 %x, 255
  %b = and i16 %y, 255
  %c = and i16 %z, 255
  %d = zext nneg i16 %a to i32
  %e = uitofp nneg i16 %b to float
  %g1 = uitofp nneg i16 %c to float
  %h1 = uitofp nneg i32 %d to float
  %i = fadd float %e, %g1
  %j = fadd float %i, %h1
  ret float %j
}
