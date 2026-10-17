define i32 @ult_arm(i32 %x, i32 %y) {
  %c = icmp ult i32 %x, 16
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @eq_arms(i32 %x, i32 %y) {
  %c = icmp eq i32 %x, 5
  %s = select i1 %c, i32 %x, i32 %x
  ret i32 %s
}

define i32 @mask_eq(i32 %x, i32 %y) {
  %a = and i32 %x, 7
  %c = icmp eq i32 %a, 5
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @ne_pow2(i32 %x, i32 %y) {
  %a = and i32 %x, 8
  %c = icmp ne i32 %a, 0
  %s = select i1 %c, i32 %y, i32 %x
  ret i32 %s
}

define i32 @trunc_cond(i32 %x) {
  %t = trunc i32 %x to i1
  %s = select i1 %t, i32 %x, i32 0
  ret i32 %s
}

define i32 @trunc_nuw_cond(i32 %x) {
  %t = trunc nuw i32 %x to i1
  %s = select i1 %t, i32 %x, i32 0
  ret i32 %s
}

define i32 @trunc_cmp(i32 %x, i32 %y) {
  %t = trunc i32 %x to i8
  %c = icmp ult i8 %t, 4
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @logical_and(i32 %x, i32 %y) {
  %a = icmp ult i32 %x, 16
  %b = icmp ugt i32 %x, 3
  %c = and i1 %a, %b
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @logical_or(i32 %x, i32 %y) {
  %a = icmp ult i32 %x, 16
  %b = icmp ult i32 %x, 4
  %c = or i1 %a, %b
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @select_form_and(i32 %x, i32 %y) {
  %a = icmp ult i32 %x, 16
  %b = icmp ugt i32 %y, -256
  %c = select i1 %a, i1 %b, i1 false
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @not_cond(i32 %x, i32 %y) {
  %a = icmp ult i32 %x, 16
  %c = xor i1 %a, true
  %s = select i1 %c, i32 %x, i32 %y
  ret i32 %s
}

define i32 @nested(i32 %x, i32 %y, i1 %k) {
  %a = icmp ult i32 %x, 16
  %i = select i1 %k, i32 %x, i32 %y
  %s = select i1 %a, i32 %i, i32 %x
  ret i32 %s
}

define i32 @branch(i32 %x, i32 %y) {
  %a = icmp ult i32 %x, 16
  br i1 %a, label %t, label %f
t:
  ret i32 %x
f:
  ret i32 %y
}

define i32 @phi_drops(i32 %x) {
entry:
  br label %loop
loop:
  %p = phi i32 [ %x, %entry ], [ %n, %loop ]
  %n = lshr i32 %p, 1
  %c = icmp ult i32 %p, 16
  %s = select i1 %c, i32 %p, i32 %n
  %d = icmp eq i32 %s, 0
  br i1 %d, label %out, label %loop
out:
  ret i32 %s
}

define i32 @phi_arm(i32 %x, i32 %y, i1 %k) {
entry:
  br i1 %k, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %p = phi i32 [ %x, %a ], [ %y, %b ]
  %c = icmp ult i32 %x, 16
  %s = select i1 %c, i32 %p, i32 %y
  ret i32 %s
}

define ptr @null_cond(ptr %p, ptr %q) {
  %c = icmp eq ptr %p, null
  %s = select i1 %c, ptr %p, ptr %q
  ret ptr %s
}
