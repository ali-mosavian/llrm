declare i32 @llvm.ctpop.i32(i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.bitreverse.i32(i32)
declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i32 @llvm.fshr.i32(i32, i32, i32)
declare i64 @llvm.vscale.i64()

define void @shifts(i32 %x, i32 %n, i32 range(i32 1, 0) %nz) {
  %a = shl i32 1, %n
  %b = lshr i32 -2147483648, %n
  %c = shl i32 2, %n
  %d = shl i32 %a, %n
  %e = shl nuw i32 %a, %n
  %f = shl nsw i32 %a, %n
  %g = lshr i32 %b, %n
  %h = lshr exact i32 %b, %n
  %i = udiv i32 %a, %n
  %j = udiv exact i32 %a, %n
  %k = zext i32 %a to i64
  %l = trunc i64 %k to i32
  %m = trunc i32 %a to i16
  %o = mul i32 %a, %a
  %p = mul nuw i32 %a, %a
  %q = mul i32 %a, %b
  %r = mul nsw i32 %a, %nz
  %s = ashr i32 %a, %n
  %t = shl i32 %x, %n
  %u = shl i1 1, 0
  ret void
}

define void @ands(i32 %x, i32 %y, i32 %n) {
  %a = shl i32 1, %n
  %neg = sub i32 0, %x
  %b = and i32 %x, %neg
  %c = and i32 %neg, %x
  %d = and i32 %a, %y
  %e = and i32 %y, %a
  %f = and i32 %x, %y
  %nega = sub i32 0, %a
  %g = and i32 %a, %nega
  %h = add i32 %a, %a
  %i = add nuw i32 %a, %a
  %j = add nsw i32 %a, %a
  %k = add i32 %a, 1
  %l = and i32 %a, %y
  %m = add i32 %l, %a
  %nn = add i32 %a, %l
  %o = lshr i32 -1, %n
  %p = add i32 %o, 1
  %q = add nuw i32 %o, 1
  %r = add nsw i32 %o, 1
  %s = or i32 %a, %a
  %t = xor i32 %a, %a
  ret void
}

define void @selects(i1 %c, i32 %n, i32 %m, i32 %x) {
  %a = shl i32 1, %n
  %b = shl i32 1, %m
  %s1 = select i1 %c, i32 %a, i32 %b
  %s2 = select i1 %c, i32 %a, i32 8
  %s3 = select i1 %c, i32 %a, i32 6
  %s4 = select i1 %c, i32 %a, i32 0
  %s5 = select i1 %c, i32 %a, i32 %x
  %i1 = and i1 %c, %c
  %i2 = xor i1 %c, true
  %z = zext i1 %c to i32
  ret void
}

define void @intrin(i32 %x, i32 %n, i32 %m) {
  %a = shl i32 1, %n
  %b = shl i32 1, %m
  %u1 = call i32 @llvm.umax.i32(i32 %a, i32 %b)
  %u2 = call i32 @llvm.umin.i32(i32 %a, i32 %b)
  %u3 = call i32 @llvm.smax.i32(i32 %a, i32 %x)
  %u4 = call i32 @llvm.bswap.i32(i32 %a)
  %u5 = call i32 @llvm.bitreverse.i32(i32 %a)
  %u6 = call i32 @llvm.fshl.i32(i32 %a, i32 %a, i32 %x)
  %u7 = call i32 @llvm.fshr.i32(i32 %a, i32 %b, i32 %x)
  %u8 = call i32 @llvm.umax.i32(i32 %a, i32 8)
  %v = call i64 @llvm.vscale.i64()
  ret void
}

define void @phis(i1 %c, i32 %n, i32 %m, i32 %x) {
entry:
  %a = shl i32 1, %n
  %b = shl i32 1, %m
  br i1 %c, label %l, label %r
l:
  br label %j
r:
  br label %j
j:
  %p1 = phi i32 [ %a, %l ], [ %b, %r ]
  %p2 = phi i32 [ %a, %l ], [ %x, %r ]
  %p3 = phi i32 [ %a, %l ], [ 4, %r ]
  %p4 = phi i32 [ %a, %l ], [ 0, %r ]
  br label %loop
loop:
  %i1 = phi i32 [ 1, %j ], [ %i1n, %loop ]
  %i1n = shl i32 %i1, 1
  %i2 = phi i32 [ 1, %j ], [ %i2n, %loop ]
  %i2n = shl nuw i32 %i2, %n
  %i3 = phi i32 [ %a, %j ], [ %i3n, %loop ]
  %i3n = mul i32 %i3, %b
  %i4 = phi i32 [ %a, %j ], [ %i4n, %loop ]
  %i4n = mul nuw i32 %i4, 4
  %i5 = phi i32 [ 64, %j ], [ %i5n, %loop ]
  %i5n = lshr i32 %i5, 1
  %i6 = phi i32 [ 64, %j ], [ %i6n, %loop ]
  %i6n = lshr exact i32 %i6, %n
  %i7 = phi i32 [ 64, %j ], [ %i7n, %loop ]
  %i7n = udiv exact i32 %i7, %b
  %i8 = phi i32 [ 64, %j ], [ %i8n, %loop ]
  %i8n = udiv i32 %i8, %b
  %i9 = phi i32 [ 64, %j ], [ %i9n, %loop ]
  %i9n = sdiv exact i32 %i9, %b
  %i10 = phi i32 [ -2147483648, %j ], [ %i10n, %loop ]
  %i10n = sdiv exact i32 %i10, %b
  %i11 = phi i32 [ 64, %j ], [ %i11n, %loop ]
  %i11n = ashr i32 %i11, 1
  %i12 = phi i32 [ -2147483648, %j ], [ %i12n, %loop ]
  %i12n = ashr exact i32 %i12, 1
  %i13 = phi i32 [ 64, %j ], [ %i13n, %loop ]
  %i13n = ashr exact i32 %i13, 1
  %i14 = phi i32 [ 3, %j ], [ %i14n, %loop ]
  %i14n = shl i32 %i14, 1
  %i15 = phi i32 [ %a, %j ], [ %i15n, %loop ]
  %i15n = mul i32 %b, %i15
  %c2 = icmp ne i32 %n, 0
  br i1 %c2, label %loop, label %exit
exit:
  ret void
}

define void @small(i1 %b, i8 %x) {
  %a = and i1 %b, true
  %z = zext i1 %b to i8
  %s = sext i1 %b to i8
  %t = trunc i8 %x to i1
  ret void
}
