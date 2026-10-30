define float @f(float %a) {
entry:
  %r = fmul float %a, 0x3FF0040000000000
  %s = fadd float %r, 0x3FF0000020000000
  ret float %s
}
