@PSX = global [8 x i16] zeroinitializer
@PSY = global [8 x i16] zeroinitializer
@DSX = global [8 x i16] zeroinitializer
@DSY = global [8 x i16] zeroinitializer
@XP1 = global [8 x i16] zeroinitializer
@YP1 = global [8 x i16] zeroinitializer

declare i16 @llvm.lrint.i16.f32(float)
declare float @llvm.sin.f32(float)

define void @f(float %a, float %b, float %c) {
b104:
  br label %b106

b106:
  %i = phi i16 [ 1, %b104 ], [ %next, %b116 ]
  %go = icmp sle i16 %i, 3
  br i1 %go, label %b109, label %b110

b109:
  %pxp = getelementptr inbounds i16, ptr @PSX, i16 %i
  %px = load i16, ptr %pxp
  %pxf = sitofp i16 %px to float
  %xm = fmul float %pxf, %a
  %xr = call i16 @llvm.lrint.i16.f32(float %xm)
  %pyp = getelementptr inbounds i16, ptr @PSY, i16 %i
  %py = load i16, ptr %pyp
  %pyf = sitofp i16 %py to float
  %ym = fmul float %pyf, %b
  %yr = call i16 @llvm.lrint.i16.f32(float %ym)
  %big = icmp sgt i16 %xr, 320
  br i1 %big, label %b113, label %b112

b112:
  br label %b113

b113:
  %xc = phi i16 [ 320, %b109 ], [ %xr, %b112 ]
  %bigy = icmp sgt i16 %yr, 300
  br i1 %bigy, label %b116, label %b115

b115:
  br label %b116

b116:
  %yc = phi i16 [ 300, %b113 ], [ %yr, %b115 ]
  %dxp = getelementptr inbounds i16, ptr @DSX, i16 %i
  %dx = load i16, ptr %dxp
  %dxf = sitofp i16 %dx to float
  %q = fdiv float %c, %dxf
  %s = call float @llvm.sin.f32(float %q)
  %xcf = sitofp i16 %xc to float
  %sx = fmul float %s, %xcf
  %sxr = call i16 @llvm.lrint.i16.f32(float %sx)
  %xp1 = getelementptr inbounds i16, ptr @XP1, i16 %i
  store i16 %sxr, ptr %xp1
  %dyp = getelementptr inbounds i16, ptr @DSY, i16 %i
  %dy = load i16, ptr %dyp
  %dyf = sitofp i16 %dy to float
  %q2 = fdiv float %c, %dyf
  %s2 = call float @llvm.sin.f32(float %q2)
  %ycf = sitofp i16 %yc to float
  %sy = fmul float %s2, %ycf
  %syr = call i16 @llvm.lrint.i16.f32(float %sy)
  %yp1 = getelementptr inbounds i16, ptr @YP1, i16 %i
  store i16 %syr, ptr %yp1
  %next = add i16 %i, 1
  br label %b106

b110:
  ret void
}
