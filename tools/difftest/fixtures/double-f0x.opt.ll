define double @f(double %a) {
entry:
  %r = fmul double %a, f0x3FF0040000000000
  ret double %r
}
