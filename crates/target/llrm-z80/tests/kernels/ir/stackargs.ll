source_filename = "stackargs.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; Nine byte arguments: seven in registers, the 8th and 9th on the stack. The callee reads them from the frame the caller stored.
define i8 @g(i8 %a, i8 %b, i8 %c, i8 %d, i8 %e, i8 %f, i8 %g, i8 %h, i8 %i) noinline {
  %h2 = shl i8 %h, 1
  %s = add i8 %a, %h2
  %t = add i8 %s, %i
  ret i8 %t
}

define i8 @stackargs(i8 %a) {
  %r = call i8 @g(i8 %a, i8 2, i8 3, i8 4, i8 5, i8 6, i8 7, i8 8, i8 9)
  ret i8 %r
}
