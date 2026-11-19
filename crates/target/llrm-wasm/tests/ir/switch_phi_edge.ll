target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@g = global i32 0
define i32 @f(i32 %x, i32 %y) {
e:
  %s = and i32 %y, 7
  switch i32 %s, label %d [
    i32 0, label %m
    i32 1, label %a
    i32 2, label %b
  ]
a:
  %p = lshr i32 %x, 16
  br label %m
b:
  %q = lshr i32 %x, 24
  br label %m
d:
  %r = lshr i32 %x, 29
  br label %m
m:
  %v = phi i32 [ %x, %e ], [ %p, %a ], [ %q, %b ], [ %r, %d ]
  %w = and i32 %v, 65535
  %z = udiv i32 %w, 10
  store volatile i32 %z, ptr @g
  %u = urem i32 %w, 7
  ret i32 %u
}
