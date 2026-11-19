target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; A 16-trip dot product inside two loops of unknown count (jpeg_fixed -O2).
define void @shape(ptr %p, ptr %q, ptr %out, i32 %n, i32 %m) {
entry:
  br label %o1
o1:
  %i = phi i32 [ 0, %entry ], [ %i1, %o1.latch ]
  br label %o2
o2:
  %j = phi i32 [ 0, %o1 ], [ %j1, %o2.latch ]
  br label %in
in:
  %k = phi i32 [ 0, %o2 ], [ %k1, %in ]
  %s = phi i32 [ 0, %o2 ], [ %s1, %in ]
  %pk = getelementptr i32, ptr %p, i32 %k
  %x = load i32, ptr %pk
  %qk = getelementptr i32, ptr %q, i32 %k
  %y = load i32, ptr %qk
  %t = mul i32 %x, %y
  %s1 = add i32 %s, %t
  %k1 = add i32 %k, 1
  %c = icmp ne i32 %k1, 16
  br i1 %c, label %in, label %o2.latch
o2.latch:
  %slot = getelementptr i32, ptr %out, i32 %j
  store i32 %s1, ptr %slot
  %j1 = add i32 %j, 1
  %cj = icmp ne i32 %j1, %m
  br i1 %cj, label %o2, label %o1.latch
o1.latch:
  %i1 = add i32 %i, 1
  %ci = icmp ne i32 %i1, %n
  br i1 %ci, label %o1, label %done
done:
  ret void
}
