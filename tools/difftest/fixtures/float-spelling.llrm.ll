define float @f(float %a) {
entry:
  %r = fmul float %a, 0x40445EB860000000
  %s = fadd float %r, 0x3FF0000000000000
  ret float %s
}
