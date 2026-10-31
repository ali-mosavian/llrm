target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %a = phi i32 [0, %entry], [%b, %loop]
  %b = phi i32 [1, %entry], [%s, %loop]
  %s = add i32 %a, %b
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s
}
