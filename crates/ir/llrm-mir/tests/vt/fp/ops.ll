define void @f(float %x, float %y, double %d, i32 %i, i16 %s, i1 %c) {
  %neg = fneg float %x
  %abs = call float @llvm.fabs.f32(float %x)
  %cs = call float @llvm.copysign.f32(float %x, float %y)
  %sel = select i1 %c, float %abs, float 1.0
  %add = fadd float %abs, 1.0
  %addn = fadd nnan float %x, %y
  %sub = fsub float %x, %y
  %sub1 = fsub ninf nnan float %x, %y
  %mul = fmul float %x, %x
  %mulc = fmul float %x, 2.0
  %mulf = fmul float %x, %y
  %fl = call float @llvm.floor.f32(float %x)
  %frac = fsub float %x, %fl
  %small = fmul ninf float %x, %frac
  %div = fdiv float %x, %y
  %divs = fdiv float %x, %x
  %divp = fdiv float %abs, 2.0
  %rem = frem float %x, %y
  %rems = frem float %x, %x
  %ext = fpext float %abs to double
  %trunc = fptrunc double %d to float
  %truncp = fptrunc double %ext to float
  %u = uitofp i32 %i to float
  %sn = sitofp i16 %s to float
  %ui = zext i16 %s to i32
  %sp = sitofp i32 %ui to double
  %up = or i32 %i, 1
  %nz = uitofp i32 %up to float
  %big = sitofp i32 %i to double
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %p = phi float [ %abs, %a ], [ 0.0, %b ]
  %q = phi float [ %x, %a ], [ %neg, %b ]
  %p2 = phi float [ %mul, %a ], [ %u, %b ]
  %nan = fadd float 0x7FF8000000000000, %x
  %pz = fadd float %p, -0.0
  %fl1 = fsub nnan ninf float %x, %y
  %pm = phi float [ %fl1, %a ], [ %sub, %b ]
  ret void
}
declare float @llvm.fabs.f32(float)
declare float @llvm.copysign.f32(float, float)
declare float @llvm.floor.f32(float)
