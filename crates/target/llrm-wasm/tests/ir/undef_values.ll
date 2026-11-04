declare i32 @use(i32, i32)

define i32 @ret_undef() {
  ret i32 undef
}

define double @ret_undef_f64() {
  ret double undef
}

define i32 @arg_undef(i32 %a) {
  %r = call i32 @use(i32 %a, i32 undef)
  ret i32 %r
}
