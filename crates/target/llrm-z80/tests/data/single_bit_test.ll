target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

declare void @sink()

define void @high_bit_of_a_long(i32 %x) {
  %a = and i32 %x, 256
  %c = icmp eq i32 %a, 0
  br i1 %c, label %yes, label %no
yes:
  call void @sink()
  ret void
no:
  ret void
}

define void @low_bit_of_a_byte(i8 %x) {
  %a = and i8 %x, 16
  %c = icmp ne i8 %a, 0
  br i1 %c, label %yes, label %no
yes:
  call void @sink()
  ret void
no:
  ret void
}
