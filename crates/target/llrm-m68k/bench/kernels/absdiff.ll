target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %a, i32 %b) {
entry:
  %c = icmp sgt i32 %a, %b
  br i1 %c, label %hi, label %lo
hi:
  %d = sub i32 %a, %b
  ret i32 %d
lo:
  %e = sub i32 %b, %a
  ret i32 %e
}
