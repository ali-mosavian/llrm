define i32 @f(i16 %x, ptr %p) {
entry:
  %c = icmp sgt i16 %x, 63
  br i1 %c, label %m, label %m.thread

m:
  %l = load i16, ptr %p
  %t = icmp eq i16 %l, 116
  br i1 %t, label %a, label %y

m.thread:
  %g1 = getelementptr i16, ptr %p, i16 %x
  %l1 = load i16, ptr %g1
  %t1 = icmp eq i16 %l1, 116
  br i1 %t1, label %a, label %n

y:
  ret i32 1

n:
  ret i32 2

a:
  ret i32 3
}
