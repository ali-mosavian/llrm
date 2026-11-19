target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @k(ptr %p) {
entry:
  br label %loop
loop:
  %i = phi i32 [0, %entry], [%i1, %loop]
  %q = getelementptr i8, ptr %p, i32 %i
  %v = load i8, ptr %q
  %z = icmp eq i8 %v, 0
  %i1 = add i32 %i, 1
  br i1 %z, label %exit, label %loop
exit:
  ret i32 %i
}
