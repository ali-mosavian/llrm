target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; A long masked to three bits and compared with 3 (the selector of div_constants_short):
; one byte `cp`, not a word compare with `sub a, #0` for the high byte the mask zeroed.
declare i32 @next()

define i16 @masked_compare() {
  %n = call i32 @next()
  %m = and i32 %n, 7
  %c = icmp eq i32 %m, 3
  %r = select i1 %c, i16 10, i16 20
  ret i16 %r
}
