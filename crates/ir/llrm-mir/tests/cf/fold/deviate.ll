declare double @llvm.sqrt.f64(double)
declare i32 @llvm.amdgcn.perm(i32, i32, i32)
declare i32 @llvm.amdgcn.s.wqm.i32(i32)
declare i32 @llvm.ctpop.i32(i32)
declare <2 x i32> @llvm.ctpop.v2i32(<2 x i32>)

define void @d() {
  %a = call double @llvm.sqrt.f64(double 4.0)
  %b = fadd double 1.0, 2.0
  %c = fneg double 1.0
  %d = fcmp olt double 1.0, 2.0
  %e = add <2 x i32> <i32 1, i32 2>, <i32 3, i32 4>
  %f = call i32 @llvm.amdgcn.perm(i32 1, i32 2, i32 3)
  %g = call i32 @llvm.amdgcn.s.wqm.i32(i32 5)
  %h = call <2 x i32> @llvm.ctpop.v2i32(<2 x i32> <i32 3, i32 7>)
  %i = sitofp i32 3 to double
  %j = fptosi double 3.5 to i32
  %k = call i32 @llvm.ctpop.i32(i32 7)
  ret void
}
