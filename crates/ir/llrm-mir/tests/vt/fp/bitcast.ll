declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)

define void @b(i32 %i, i32 %j, i64 %l, float %x, float %y, i1 %c) {
  %pos = and i32 %i, 2147483647
  %f0 = bitcast i32 %pos to float
  %neg = or i32 %i, -2147483648
  %f1 = bitcast i32 %neg to float
  %one = or i32 %i, 1065353216
  %m = and i32 %one, 1073741823
  %f2 = bitcast i32 %m to float
  %nan = or i32 %i, 2143289344
  %f3 = bitcast i32 %nan to float
  %inf = bitcast i32 2139095040 to float
  %fz = bitcast i32 0 to float
  %hi = lshr i64 %l, 12
  %hm = or i64 %hi, 4611686018427387904
  %d0 = bitcast i64 %hm to double
  %bx = bitcast float %x to i32
  %by = bitcast float %y to i32
  %mx = call i32 @llvm.umax.i32(i32 %bx, i32 %by)
  %fm = bitcast i32 %mx to float
  %mn = call i32 @llvm.smin.i32(i32 %bx, i32 %by)
  %fn = bitcast i32 %mn to float
  %cmp = icmp ugt i32 %bx, %by
  %sel = select i1 %cmp, i32 %bx, i32 %by
  %fs = bitcast i32 %sel to float
  %ax = call float @llvm.fabs.f32(float %x)
  %ay = call float @llvm.fabs.f32(float %y)
  %bax = bitcast float %ax to i32
  %bay = bitcast float %ay to i32
  %mxa = call i32 @llvm.umax.i32(i32 %bax, i32 %bay)
  %fma = bitcast i32 %mxa to float
  ret void
}
declare float @llvm.fabs.f32(float)
