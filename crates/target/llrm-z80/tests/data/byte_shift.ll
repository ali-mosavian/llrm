target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@stream = global [16 x i8] zeroinitializer, align 1

; huffman_decode's getbit: bit (7 - pos & 7) of a stream byte, the byte promoted to int.
define i16 @getbit(i16 %pos) {
  %i = lshr i16 %pos, 3
  %p = getelementptr inbounds [16 x i8], ptr @stream, i16 0, i16 %i
  %b = load i8, ptr %p, align 1
  %z = zext i8 %b to i16
  %m = and i16 %pos, 7
  %k = sub nsw i16 7, %m
  %s = lshr i16 %z, %k
  %r = and i16 %s, 1
  ret i16 %r
}
