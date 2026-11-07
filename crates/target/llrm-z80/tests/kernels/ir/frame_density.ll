target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; A cold 100-byte array (one use) and a hot 120-byte one (eight stores): by size alone the cold one
; lay nearest IX and the hot one beyond its reach.
declare void @touch(ptr)

define void @density(i8 %v) {
  %cold = alloca [100 x i8]
  %hot = alloca [120 x i8]
  call void @touch(ptr %cold)
  %h0 = getelementptr [120 x i8], ptr %hot, i16 0, i16 0
  %h1 = getelementptr [120 x i8], ptr %hot, i16 0, i16 1
  %h2 = getelementptr [120 x i8], ptr %hot, i16 0, i16 2
  %h3 = getelementptr [120 x i8], ptr %hot, i16 0, i16 3
  %h4 = getelementptr [120 x i8], ptr %hot, i16 0, i16 4
  %h5 = getelementptr [120 x i8], ptr %hot, i16 0, i16 5
  %h6 = getelementptr [120 x i8], ptr %hot, i16 0, i16 6
  %h7 = getelementptr [120 x i8], ptr %hot, i16 0, i16 7
  store volatile i8 %v, ptr %h0
  store volatile i8 %v, ptr %h1
  store volatile i8 %v, ptr %h2
  store volatile i8 %v, ptr %h3
  store volatile i8 %v, ptr %h4
  store volatile i8 %v, ptr %h5
  store volatile i8 %v, ptr %h6
  store volatile i8 %v, ptr %h7
  call void @touch(ptr %hot)
  ret void
}
