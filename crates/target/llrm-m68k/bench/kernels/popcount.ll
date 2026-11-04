target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %x) {
entry:
  br label %loop
loop:
  %v = phi i32 [%x, %entry], [%v1, %loop]
  %c = phi i32 [0, %entry], [%c1, %loop]
  %b = and i32 %v, 1
  %c1 = add i32 %c, %b
  %v1 = lshr i32 %v, 1
  %z = icmp eq i32 %v1, 0
  br i1 %z, label %exit, label %loop
exit:
  ret i32 %c1
}
