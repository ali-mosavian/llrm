declare i8 @llvm.smin.i8(i8, i8)
declare i8 @llvm.smax.i8(i8, i8)
declare i8 @llvm.umin.i8(i8, i8)
declare i8 @llvm.umax.i8(i8, i8)

define void @minmax(i8 %x, i8 %y) {
  %c1 = icmp slt i8 %x, 10
  %a = select i1 %c1, i8 %x, i8 10
  %c2 = icmp sgt i8 %x, 10
  %b = select i1 %c2, i8 %x, i8 10
  %c3 = icmp ult i8 %x, 10
  %c = select i1 %c3, i8 %x, i8 10
  %c4 = icmp ugt i8 %x, 10
  %d = select i1 %c4, i8 %x, i8 10
  %c5 = icmp sle i8 10, %x
  %e = select i1 %c5, i8 10, i8 %x
  %c6 = icmp uge i8 %y, %x
  %f = select i1 %c6, i8 %y, i8 %x
  %c7 = icmp eq i8 %x, 3
  %g = select i1 %c7, i8 %x, i8 7
  ret void
}

define void @abs(i8 %x) {
  %n = sub i8 0, %x
  %nn = sub nsw i8 0, %x
  %c1 = icmp sgt i8 %x, -1
  %a = select i1 %c1, i8 %x, i8 %n
  %b = select i1 %c1, i8 %x, i8 %nn
  %c2 = icmp slt i8 %x, 0
  %c = select i1 %c2, i8 %nn, i8 %x
  %d = select i1 %c2, i8 %x, i8 %nn
  %e = select i1 %c1, i8 %n, i8 %x
  %c3 = icmp sge i8 %x, 1
  %f = select i1 %c3, i8 %x, i8 %nn
  %c4 = icmp slt i8 %x, 1
  %g = select i1 %c4, i8 %x, i8 %nn
  %c5 = icmp sgt i8 %nn, 0
  %h = select i1 %c5, i8 %nn, i8 %x
  ret void
}

define void @absext(i8 %x) {
  %s = sext i8 %x to i16
  %n = sub i16 0, %s
  %c = icmp sgt i8 %x, 0
  %cx = sext i8 %x to i16
  %c1 = icmp sgt i16 %cx, 0
  %a = select i1 %c1, i16 %s, i16 %n
  %m = mul nsw i16 %cx, 3
  %nm = sub i16 0, %m
  %b = select i1 %c1, i16 %m, i16 %nm
  %sh = shl nsw i16 %cx, 2
  %nsh = sub i16 0, %sh
  %d = select i1 %c1, i16 %sh, i16 %nsh
  ret void
}

define void @clamp(i8 %x) {
  %lo = call i8 @llvm.smin.i8(i8 %x, i8 50)
  %c1 = icmp slt i8 %x, 10
  %a = select i1 %c1, i8 10, i8 %lo
  %hi = call i8 @llvm.smax.i8(i8 %x, i8 -50)
  %c2 = icmp sgt i8 %x, 10
  %b = select i1 %c2, i8 10, i8 %hi
  %ulo = call i8 @llvm.umin.i8(i8 %x, i8 50)
  %c3 = icmp ult i8 %x, 10
  %c = select i1 %c3, i8 10, i8 %ulo
  %uhi = call i8 @llvm.umax.i8(i8 %x, i8 5)
  %c4 = icmp ugt i8 %x, 10
  %d = select i1 %c4, i8 10, i8 %uhi
  ret void
}

define void @minmaxofminmax(i8 %a, i8 %b, i8 %c) {
  %c1 = icmp slt i8 %a, %b
  %m1 = select i1 %c1, i8 %a, i8 %b
  %c2 = icmp slt i8 %b, %c
  %m2 = select i1 %c2, i8 %b, i8 %c
  %c3 = icmp slt i8 %m1, %m2
  %r = select i1 %c3, i8 %m1, i8 %m2
  %c4 = icmp slt i8 %a, %c
  %p = select i1 %c4, i8 %m1, i8 %m2
  ret void
}

define void @nots(i8 %x, i8 %y) {
  %nx = xor i8 %x, -1
  %ny = xor i8 -1, %y
  %c1 = icmp sgt i8 %x, %y
  %a = select i1 %c1, i8 %nx, i8 %ny
  %c2 = icmp slt i8 %x, %y
  %b = select i1 %c2, i8 %ny, i8 %nx
  %c3 = icmp ult i8 %x, 5
  %nx2 = xor i8 %x, -1
  %c = select i1 %c3, i8 %nx2, i8 -6
  ret void
}

define void @signedunsigned(i8 %x) {
  %c1 = icmp slt i8 %x, 0
  %a = select i1 %c1, i8 %x, i8 127
  %b = select i1 %c1, i8 127, i8 %x
  %c2 = icmp sgt i8 %x, -1
  %c = select i1 %c2, i8 -128, i8 %x
  %d = select i1 %c2, i8 %x, i8 -128
  ret void
}

define void @selects(i1 %c, i8 %x) {
  %a = select i1 %c, i8 3, i8 9
  %b = select i1 %c, i8 %x, i8 9
  %t = and i8 %x, 15
  %d = select i1 %c, i8 %t, i8 100
  ret void
}

define void @casts(i32 %x, i64 %y) {
  %t = trunc i64 %y to i32
  %u = urem i64 %y, 100
  %v = trunc i64 %u to i8
  %w = and i32 %x, 1023
  %z = trunc i32 %w to i16
  ret void
}
