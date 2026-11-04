source_filename = "carry32.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; The high word of a + b: the carry out of the low words, a compare used as a value.
define i16 @carry32(i32 %a, i8 %b) {
  %w = zext i8 %b to i32
  %s = add i32 %a, %w
  %h = lshr i32 %s, 16
  %r = trunc i32 %h to i16
  ret i16 %r
}
