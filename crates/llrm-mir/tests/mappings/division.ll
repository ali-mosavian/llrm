; Division is C's: sdiv and srem, unguarded.
; expect: 4
target datalayout = "e-p:16:16-p1:32:16:16:16-i32:16-i64:16"

define i32 @main() {
  %q = sdiv i16 10, 3
  %r = srem i16 10, 3
  %t = add i16 %q, %r
  %w = sext i16 %t to i32
  ret i32 %w
}
