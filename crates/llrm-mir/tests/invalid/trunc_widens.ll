; invalid: cannot take i16 to i32
define i32 @f(i16 %x) {
  %y = trunc i16 %x to i32
  ret i32 %y
}
