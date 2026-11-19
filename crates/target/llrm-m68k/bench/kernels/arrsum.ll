target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(ptr %p, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %s = phi i32 [0, %entry], [%s1, %loop]
  %q = getelementptr i32, ptr %p, i32 %i
  %v = load i32, ptr %q
  %s1 = add i32 %s, %v
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %s1
}
