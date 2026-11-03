source_filename = "indirect.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

@buf = external global ptr

define i16 @dbl(i16 %x) noinline {
  %r = shl i16 %x, 1
  ret i16 %r
}

; A call through a pointer: the address goes in iy and the call is the runtime's `jp (iy)`.
define i16 @indirect(i16 %x) {
  store volatile ptr @dbl, ptr @buf
  %f = load volatile ptr, ptr @buf
  %r = call i16 %f(i16 %x)
  ret i16 %r
}
