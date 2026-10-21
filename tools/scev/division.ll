; sdiv operands for SCEVDivision: each division is one case of divide().
define void @constants(i32 %x) {
  %a = sdiv i32 17, 5
  %b = sdiv i32 -17, 5
  %c = sdiv i32 0, 5
  %d = sdiv i32 8, 8
  %e = sdiv i32 %x, 1
  %f = sdiv i32 %x, %x
  ret void
}

define void @products(i32 %a, i32 %b, i32 %c) {
  %ab = mul i32 %a, %b
  %abc = mul i32 %ab, %c
  %n8 = mul i32 %abc, 8
  %d = mul i32 %b, %c
  %p = sdiv i32 %n8, %d
  %q = sdiv i32 %ab, %a
  %r = sdiv i32 %n8, %ab
  %s = sdiv i32 %ab, %c
  %t = sdiv i32 %n8, 4
  %u = sdiv i32 %n8, 3
  ret void
}

define void @sums(i32 %a, i32 %b, i32 %c) {
  %ab = mul i32 %a, %b
  %ac = mul i32 %a, %c
  %s = add i32 %ab, %ac
  %t = add i32 %s, %b
  %p = sdiv i32 %s, %a
  %q = sdiv i32 %t, %a
  %r = sdiv i32 %s, %b
  %u = add i32 %ab, 6
  %v = sdiv i32 %u, 3
  ret void
}

define void @addrec(i32 %m, i32 %o) {
entry:
  br label %i
i:
  %i.iv = phi i32 [ 0, %entry ], [ %i.next, %i.latch ]
  br label %j
j:
  %j.iv = phi i32 [ 0, %i ], [ %j.next, %j ]
  %mo = mul i32 %m, %o
  %i8 = mul i32 %i.iv, %mo
  %i8x = mul i32 %i8, 8
  %j8 = mul i32 %j.iv, %o
  %j8x = mul i32 %j8, 8
  %off = add i32 %i8x, %j8x
  %off2 = add i32 %off, 16
  %d1 = sdiv i32 %off2, %mo
  %d2 = sdiv i32 %off2, 8
  %d3 = sdiv i32 %off2, %o
  %d4 = sdiv i32 %j8x, %o
  %j.next = add nuw nsw i32 %j.iv, 1
  %jc = icmp slt i32 %j.next, %m
  br i1 %jc, label %j, label %i.latch
i.latch:
  %i.next = add nuw nsw i32 %i.iv, 1
  %ic = icmp slt i32 %i.next, %m
  br i1 %ic, label %i, label %exit
exit:
  ret void
}

define void @unknowns(i32 %a, i32 %b, i32 %c) {
  %ab = mul i32 %a, %b
  %t = add i32 %ab, %c
  %u = add i32 %t, 4
  %p = sdiv i32 %t, %a
  %q = sdiv i32 %u, %a
  %w = mul i32 %t, %a
  %r = sdiv i32 %w, %a
  %z = zext i32 %a to i64
  %zz = mul i64 %z, 4
  %s = sdiv i64 %zz, 4
  ret void
}
