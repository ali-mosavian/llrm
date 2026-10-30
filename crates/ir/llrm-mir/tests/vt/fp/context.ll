declare void @llvm.assume(i1)
declare i1 @llvm.is.fpclass.f32(float, i32)
declare float @llvm.fabs.f32(float)

define void @h(float %x, float %y, float %z, i1 %c, i32 %bits) {
entry:
  %lt = fcmp olt float %x, 0.0
  br i1 %lt, label %neg, label %nonneg
neg:
  %nx = fadd float %x, 1.0
  %nm = fmul float %x, %x
  br label %join
nonneg:
  %px = fadd float %x, 1.0
  %pa = call float @llvm.fabs.f32(float %x)
  br label %join
join:
  %j = phi float [ %nx, %neg ], [ %px, %nonneg ]
  %fin = fcmp one float %y, 0x7FF0000000000000
  %ord = fcmp ord float %y, 0.0
  %both = and i1 %fin, %ord
  br i1 %both, label %fine, label %other
fine:
  %fy = fadd float %y, %y
  %fy2 = fmul float %y, 2.0
  br label %other
other:
  %cls = call i1 @llvm.is.fpclass.f32(float %z, i32 3)
  br i1 %cls, label %nan, label %num
nan:
  %zz = fadd float %z, 1.0
  br label %num
num:
  call void @llvm.assume(i1 %lt)
  %big = fcmp ogt float %z, 1.0
  call void @llvm.assume(i1 %big)
  %az = fadd float %z, 2.0
  %sel = select i1 %lt, float %x, float 1.0
  %ab = bitcast float %y to i32
  %sgn = icmp slt i32 %ab, 0
  br i1 %sgn, label %s1, label %s0
s1:
  %m1 = fadd float %y, 0.0
  br label %s0
s0:
  ret void
}
