declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)

define i32 @rec_shl(i32 %n) {
entry:
  br label %loop
loop:
  %iv = phi i32 [ 16, %entry ], [ %next, %loop ]
  %next = shl i32 %iv, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %iv
}

define i32 @rec_lshr(i32 %n, i32 %s) {
entry:
  %s0 = and i32 %s, 255
  br label %loop
loop:
  %iv = phi i32 [ %s0, %entry ], [ %next, %loop ]
  %next = lshr i32 %iv, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %iv
}

define i32 @rec_ashr(i32 %n, i32 %s) {
entry:
  %s0 = or i32 %s, -256
  br label %loop
loop:
  %iv = phi i32 [ %s0, %entry ], [ %next, %loop ]
  %next = ashr i32 %iv, 1
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %iv
}

define i32 @rec_udiv_urem(i32 %n, i32 %s) {
entry:
  %s0 = and i32 %s, 1023
  br label %loop
loop:
  %iv = phi i32 [ %s0, %entry ], [ %next, %loop ]
  %iv2 = phi i32 [ %s0, %entry ], [ %next2, %loop ]
  %iv3 = phi i32 [ %s0, %entry ], [ %next3, %loop ]
  %next = udiv i32 %iv, %n
  %next2 = urem i32 %iv2, %n
  %next3 = udiv i32 %n, %iv3
  %c = icmp ult i32 %next, %n
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %iv
}

define i32 @rec_add_sub_mul(i32 %n, i32 %s, i32 %t) {
entry:
  %s0 = and i32 %s, 255
  %s1 = shl i32 %s0, 4
  %t0 = shl i32 %t, 3
  %t1 = and i32 %t0, 120
  br label %loop
loop:
  %a = phi i32 [ %s1, %entry ], [ %an, %loop ]
  %b = phi i32 [ %s1, %entry ], [ %bn, %loop ]
  %c = phi i32 [ %s1, %entry ], [ %cn, %loop ]
  %d = phi i32 [ %s1, %entry ], [ %dn, %loop ]
  %e = phi i32 [ %s1, %entry ], [ %en, %loop ]
  %f = phi i32 [ %s1, %entry ], [ %fn, %loop ]
  %an = add nsw i32 %a, %t1
  %bn = sub nsw i32 %b, %t1
  %cn = mul nsw i32 %c, %t1
  %dn = and i32 %d, %t0
  %en = or i32 %t0, %e
  %fn = sub nsw i32 %t1, %f
  %cc = icmp ult i32 %an, %n
  br i1 %cc, label %loop, label %exit
exit:
  ret i32 %a
}

define i32 @rec_neg(i32 %n, i32 %s, i32 %t) {
entry:
  %s0 = or i32 %s, -2147483648
  %t0 = or i32 %t, -2147483648
  br label %loop
loop:
  %a = phi i32 [ %s0, %entry ], [ %an, %loop ]
  %b = phi i32 [ %s0, %entry ], [ %bn, %loop ]
  %an = add nsw i32 %a, %t0
  %t1 = and i32 %t, 255
  %bn = sub nsw i32 %b, %t1
  %cc = icmp ult i32 %an, %n
  br i1 %cc, label %loop, label %exit
exit:
  ret i32 %a
}

define i32 @rec_minmax(i32 %n, i32 %s, i32 %t) {
entry:
  %s0 = and i32 %s, 255
  %t0 = and i32 %t, 4095
  br label %loop
loop:
  %a = phi i32 [ %s0, %entry ], [ %an, %loop ]
  %b = phi i32 [ %s0, %entry ], [ %bn, %loop ]
  %an = call i32 @llvm.umin.i32(i32 %a, i32 %t0)
  %bn = call i32 @llvm.umax.i32(i32 %t0, i32 %b)
  %cc = icmp ult i32 %an, %n
  br i1 %cc, label %loop, label %exit
exit:
  ret i32 %a
}

define i32 @phi_merge(i1 %c, i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = and i32 %y, 7
  br i1 %c, label %l, label %r
l:
  br label %m
r:
  br label %m
m:
  %p = phi i32 [ %a, %l ], [ %b, %r ]
  %q = phi i32 [ 5, %l ], [ 7, %r ]
  ret i32 %p
}

define i32 @phi_cond(i32 %x) {
entry:
  %c = icmp ult i32 %x, 100
  br i1 %c, label %t, label %e
t:
  br label %m
e:
  br label %m
m:
  %p = phi i32 [ %x, %t ], [ 50, %e ]
  ret i32 %p
}

define i32 @phi_cond_false(i32 %x) {
entry:
  %c = icmp uge i32 %x, 100
  br i1 %c, label %e, label %t
t:
  br label %m
e:
  br label %m
m:
  %p = phi i32 [ %x, %t ], [ 120, %e ]
  ret i32 %p
}

define i32 @phi_direct_cond(i32 %x, i32 %y) {
entry:
  %xa = and i32 %y, 8
  %c = icmp ult i32 %x, 16
  br i1 %c, label %m, label %e
e:
  br label %m
m:
  %p = phi i32 [ %x, %entry ], [ 3, %e ]
  ret i32 %p
}

define i32 @phi_self_select(i1 %c, i32 %x) {
entry:
  %a = and i32 %x, 15
  br label %loop
loop:
  %p = phi i32 [ %a, %entry ], [ %s, %loop ]
  %add = or i32 %p, 16
  %s = select i1 %c, i32 %p, i32 %add
  br i1 %c, label %loop, label %exit
exit:
  ret i32 %p
}

define i32 @phi_two_phis(i1 %c, i32 %x) {
entry:
  %a = and i32 %x, 15
  br label %loop
loop:
  %p = phi i32 [ %a, %entry ], [ %q, %loop2 ]
  br i1 %c, label %loop2, label %exit
loop2:
  %q = phi i32 [ %p, %loop ]
  br label %loop
exit:
  ret i32 %p
}

define i32 @phi_const_same(i1 %c) {
entry:
  br i1 %c, label %l, label %r
l:
  br label %m
r:
  br label %m
m:
  %p = phi i32 [ 5, %l ], [ 5, %r ]
  ret i32 %p
}
