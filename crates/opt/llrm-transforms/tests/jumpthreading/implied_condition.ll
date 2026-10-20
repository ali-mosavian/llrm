; processImpliedCondition: a < b is true on the way into %m, so the test of a <= b in %b is true; LazyValueInfo has no range for either side.
define i32 @f(i32 %a, i32 %b, ptr %z) {
entry:
  %c = icmp slt i32 %a, %b
  br i1 %c, label %m, label %n

m:
  %l = load i32, ptr %z
  %t = icmp eq i32 %l, 116
  br i1 %t, label %x, label %w

w:
  %d = icmp sle i32 %a, %b
  br i1 %d, label %y, label %n

y:
  ret i32 1

n:
  ret i32 2

x:
  ret i32 3
}
