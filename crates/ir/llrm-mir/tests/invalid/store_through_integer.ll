; invalid: stores through i16
define void @f(i16 %p) {
  store i16 1, i16 %p
  ret void
}
