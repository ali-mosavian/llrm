define i32 @f(i32 %a, i32 %b, ptr %z) {
entry:
  %c = icmp slt i32 %a, %b
  br i1 %c, label %m, label %n

m:
  %l = load i32, ptr %z
  %t = icmp eq i32 %l, 116
  br i1 %t, label %x, label %y

y:
  %d = icmp sle i32 %a, %b
  ret i32 1

n:
  ret i32 2

x:
  ret i32 3
}
