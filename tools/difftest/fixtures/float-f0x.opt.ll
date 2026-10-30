define float @f(float %a) {
entry:
  %r = fmul float %a, f0x3F802000
  %s = fadd float %r, f0x3F800001
  ret float %s
}
