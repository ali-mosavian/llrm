declare i32 @llvm.abs.i32(i32, i1)
declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)

define i32 @ext(i8 %a, i16 %b, i32 %c) {
  %sa = sext i8 %a to i32
  %sb = sext i16 %b to i32
  %sh = ashr i32 %c, 7
  %shl = shl i32 %sa, 3
  %shl.big = shl i32 %sa, 30
  %and = and i32 %sa, %sb
  %or = or i32 %sa, %sh
  %xor = xor i32 %sb, %sh
  %add = add i32 %sa, %sb
  %addm1u = add i32 %c, -1
  %sub = sub i32 %sa, %sb
  %mul = mul i32 %sa, %sb
  %mul2 = mul i32 %sa, %sa
  %sd = sdiv i32 %sb, 16
  %sd0 = sdiv i32 %sb, -16
  %sr = srem i32 %c, 16
  %sr2 = srem i32 %sa, 1000
  %tr = trunc i32 %sa to i16
  %tr2 = trunc i32 %sb to i16
  %tr3 = trunc i32 %c to i8
  ret i32 %sa
}

define i32 @sel(i1 %f, i8 %a, i16 %b, i32 %c) {
  %sa = sext i8 %a to i32
  %sb = sext i16 %b to i32
  %s = select i1 %f, i32 %sa, i32 %sb
  %s1 = select i1 %f, i32 %c, i32 %sb
  %lt = icmp slt i32 %c, 100
  %abs = call i32 @llvm.abs.i32(i32 %sb, i1 false)
  %abs1 = call i32 @llvm.abs.i32(i32 %c, i1 true)
  %mn = call i32 @llvm.smin.i32(i32 %c, i32 255)
  %mx = call i32 @llvm.smax.i32(i32 %mn, i32 -256)
  %mx2 = call i32 @llvm.smax.i32(i32 %c, i32 -256)
  %mn2 = call i32 @llvm.smin.i32(i32 %mx2, i32 255)
  ret i32 %s
}

define i32 @loop(i1 %f, i8 %a, i16 %b, i32 %c) {
entry:
  br i1 %f, label %l, label %r
l:
  %sa = sext i8 %a to i32
  br label %m
r:
  %sb = sext i16 %b to i32
  br label %m
m:
  %p = phi i32 [ %sa, %l ], [ %sb, %r ]
  %p1 = phi i32 [ %sa, %l ], [ %c, %r ]
  ret i32 %p
}
