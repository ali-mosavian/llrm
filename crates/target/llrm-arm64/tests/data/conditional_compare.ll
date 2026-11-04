target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

define i32 @eq_or(i32 %x) {
  %a = icmp eq i32 %x, 72
  %b = icmp eq i32 %x, 79
  %c = or i1 %a, %b
  %r = select i1 %c, i32 2, i32 1
  ret i32 %r
}

define i32 @and_and(i32 %x, i32 %y) {
  %a = icmp sgt i32 %x, 3
  %b = icmp slt i32 %y, 9
  %c = and i1 %a, %b
  %r = select i1 %c, i32 %x, i32 %y
  ret i32 %r
}

define i32 @mixed(i32 %x, i32 %y, i32 %z) {
  %a = icmp eq i32 %x, 5
  %b = icmp ne i32 %y, 7
  %c = icmp ugt i32 %z, 100
  %d = and i1 %a, %b
  %e = or i1 %d, %c
  %r = select i1 %e, i32 %x, i32 %z
  ret i32 %r
}

define i32 @negative_and_wide_constants(i32 %x, i32 %y) {
  %a = icmp sgt i32 %x, -5
  %b = icmp ult i32 %y, 31
  %c = icmp ne i32 %x, 32
  %d = icmp sle i32 %y, -31
  %e = or i1 %a, %b
  %f = and i1 %c, %d
  %g = or i1 %e, %f
  %r = select i1 %g, i32 %x, i32 %y
  ret i32 %r
}

define i32 @three_ors(i32 %x, i32 %y, i32 %z) {
  %a = icmp slt i32 %x, %y
  %b = icmp uge i32 %y, %z
  %c = icmp eq i32 %z, -32
  %d = or i1 %a, %b
  %e = or i1 %d, %c
  %r = select i1 %e, i32 %x, i32 %z
  ret i32 %r
}

define i64 @wide(i64 %x, i64 %y) {
  %a = icmp sge i64 %x, %y
  %b = icmp ne i64 %x, 0
  %c = and i1 %a, %b
  %r = select i1 %c, i64 %x, i64 %y
  ret i64 %r
}

define i32 @narrow(i8 %x, i8 %y) {
  %a = icmp sgt i8 %x, -3
  %b = icmp ult i8 %y, 200
  %c = and i1 %a, %b
  %r = select i1 %c, i32 1, i32 0
  ret i32 %r
}

define i64 @shared_leaf(i32 %x, ptr %p) {
  %a = icmp eq i32 %x, 72
  %b = icmp eq i32 %x, 79
  %c = or i1 %a, %b
  %r = select i1 %c, i64 2, i64 1
  store i64 %r, ptr %p
  %s = select i1 %a, i64 16, i64 8
  ret i64 %s
}
