; processZExt, processUIToFP and processSIToFP.
define float @f(i16 %x, i16 %y, i16 %z) {
entry:
  %a = and i16 %x, 255
  %b = and i16 %y, 255
  %c = and i16 %z, 255
  %d = zext i16 %a to i32
  %e = uitofp i16 %b to float
  %g = sitofp i16 %c to float
  %h = sitofp i32 %d to float
  %i = fadd float %e, %g
  %j = fadd float %i, %h
  ret float %j
}
