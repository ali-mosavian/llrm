define float @f(float %a) {
entry:
  %r = fmul float %a, 4.074000e+01
  %s = fadd float %r, 1.000000e+00
  ret float %s
}
