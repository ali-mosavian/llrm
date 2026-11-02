target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(i32 %a, i32 %b) {
entry:
  br label %loop
loop:
  %x = phi i32 [%a, %entry], [%x2, %next]
  %y = phi i32 [%b, %entry], [%y2, %next]
  %eq = icmp eq i32 %x, %y
  br i1 %eq, label %exit, label %next
next:
  %gt = icmp sgt i32 %x, %y
  %d = sub i32 %x, %y
  %e = sub i32 %y, %x
  %x2 = select i1 %gt, i32 %d, i32 %x
  %y2 = select i1 %gt, i32 %y, i32 %e
  br label %loop
exit:
  ret i32 %x
}
