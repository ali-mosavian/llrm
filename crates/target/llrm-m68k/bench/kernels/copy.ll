target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(ptr %d, ptr %s, i32 %n) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %sp = getelementptr i8, ptr %s, i32 %i
  %v = load i8, ptr %sp
  %dp = getelementptr i8, ptr %d, i32 %i
  store i8 %v, ptr %dp
  %i1 = add i32 %i, 1
  %c = icmp slt i32 %i1, %n
  br i1 %c, label %loop, label %exit
exit:
  %l = getelementptr i8, ptr %d, i32 %n
  %e = getelementptr i8, ptr %l, i32 -1
  %r = load i8, ptr %e
  %z = zext i8 %r to i32
  ret i32 %z
}
