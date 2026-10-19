; maybethreadThroughTwoBasicBlocks: %c is true on the edge from %entry into %m and false on the one from %z, so each gets a copy of %m that skips the test of %c in %b.
define i32 @f(i16 %x, ptr %p) {
entry:
  %c = icmp sgt i16 %x, 63
  br i1 %c, label %z, label %m

z:
  br label %m

m:
  %v = phi i16 [ %x, %entry ], [ 0, %z ]
  %g = getelementptr i16, ptr %p, i16 %v
  %l = load i16, ptr %g
  %t = icmp eq i16 %l, 116
  br i1 %t, label %a, label %b

b:
  br i1 %c, label %y, label %n

y:
  ret i32 1

n:
  ret i32 2

a:
  ret i32 3
}
