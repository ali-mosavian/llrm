define void @fptosi(float %a, float %b, double %d) {
  %ia = fptosi float %a to i32
  %ib = fptosi float %b to i32
  %c1 = fcmp olt float %a, %b
  %r1 = select i1 %c1, i32 %ia, i32 %ib
  %c2 = fcmp nnan olt float %a, %b
  %r2 = select i1 %c2, i32 %ia, i32 %ib
  %c3 = fcmp ugt float %a, %b
  %r3 = select i1 %c3, i32 %ia, i32 %ib
  %c4 = fcmp nnan ugt float %a, %b
  %r4 = select i1 %c4, i32 %ia, i32 %ib
  %c5 = fcmp nnan oge float %a, %b
  %r5 = select i1 %c5, i32 %ib, i32 %ia
  %c6 = fcmp olt float %a, 5.0
  %r6 = select i1 %c6, i32 %ia, i32 5
  %c7 = fcmp ugt float %a, 5.0
  %r7 = select i1 %c7, i32 %ia, i32 5
  %c8 = fcmp olt float %a, 5.0
  %r8 = select i1 %c8, i32 5, i32 %ia
  %c9 = fcmp olt float %a, 5.0
  %r9 = select i1 %c9, i32 %ia, i32 6
  %c10 = fcmp olt float 5.0, %a
  %r10 = select i1 %c10, i32 5, i32 %ia
  %id = fptosi double %d to i32
  %c11 = fcmp nnan ole double %d, 1.0
  %r11 = select i1 %c11, i32 %id, i32 1
  %c12 = fcmp oeq float %a, %b
  %r12 = select i1 %c12, i32 %ia, i32 %ib
  %c13 = fcmp olt float %a, 0.0
  %r13 = select i1 %c13, i32 %ia, i32 0
  %c14 = fcmp olt float %a, -0.0
  %r14 = select i1 %c14, i32 %ia, i32 0
  %ua = fptoui float %a to i32
  %c15 = fcmp ogt float %a, 7.5
  %r15 = select i1 %c15, i32 %ua, i32 7
  %c16 = fcmp ogt float %a, 7.0
  %r16 = select i1 %c16, i32 %ua, i32 7
  %c17 = fcmp ogt float %a, -1.0
  %r17 = select i1 %c17, i32 %ua, i32 -1
  ret void
}

define void @clampf(float %a) {
  %ia = fptosi float %a to i32
  %c0 = fcmp olt float %a, 9.0
  %m = select i1 %c0, float %a, float 9.0
  %im = fptosi float %m to i32
  %c1 = fcmp nnan olt float %a, 5.0
  %r1 = select i1 %c1, i32 5, i32 %im
  %c2 = fcmp nnan ogt float %a, 5.0
  %r2 = select i1 %c2, i32 5, i32 %im
  %c3 = fcmp nnan ogt float %a, 1.0
  %mx = select i1 %c3, float %a, float 1.0
  %imx = fptosi float %mx to i32
  %r3 = select i1 %c3, i32 5, i32 %imx
  %c4 = fcmp nnan ogt float %a, 5.0
  %r4 = select i1 %c4, i32 5, i32 %imx
  ret void
}

define void @ints(i8 %y, i32 %x, i16 %w, i64 %v) {
  %s = sext i8 %y to i32
  %z = zext i8 %y to i32
  %t = trunc i32 %x to i8
  %c1 = icmp slt i8 %y, 10
  %r1 = select i1 %c1, i32 %s, i32 10
  %c2 = icmp ult i8 %y, 10
  %r2 = select i1 %c2, i32 %z, i32 10
  %c3 = icmp slt i8 %y, 10
  %r3 = select i1 %c3, i32 %z, i32 10
  %c4 = icmp ult i8 %y, 10
  %r4 = select i1 %c4, i32 %s, i32 10
  %c5 = icmp slt i8 %y, -10
  %r5 = select i1 %c5, i32 %s, i32 -10
  %c6 = icmp slt i8 %y, -10
  %r6 = select i1 %c6, i32 %s, i32 300
  %c7 = icmp slt i32 %x, 100
  %r7 = select i1 %c7, i8 %t, i8 100
  %c8 = icmp slt i32 %x, 300
  %r8 = select i1 %c8, i8 %t, i8 44
  %c9 = icmp slt i32 %x, 300
  %r9 = select i1 %c9, i8 %t, i8 %y
  %c10 = icmp ult i16 %w, 9
  %zw = zext i16 %w to i32
  %r10 = select i1 %c10, i32 9, i32 %zw
  %c11 = icmp sgt i16 %w, 9
  %sw = sext i16 %w to i64
  %r11 = select i1 %c11, i64 %sw, i64 9
  %zs = zext i8 %y to i32
  %zz = zext i8 %y to i32
  %c12 = icmp ult i8 %y, 3
  %r12 = select i1 %c12, i32 %zs, i32 %zz
  %yy = sext i8 %y to i64
  %c13 = icmp slt i32 %x, 3
  %xx = sext i32 %x to i64
  %r13 = select i1 %c13, i64 %xx, i64 %yy
  %c14 = icmp slt i64 %v, 0
  %vt = trunc i64 %v to i32
  %vs = sext i32 %vt to i64
  %vz = zext i32 %vt to i64
  %r14 = select i1 %c14, i64 %vs, i64 %vz
  ret void
}
