target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"x\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %1 = alloca [6000 x i8]
  call void @llvm.lifetime.start.p0(i64 6000, ptr nonnull %1)
  %2 = addrspacecast ptr %1 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %1, i8 0, i16 6000, i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$SMID(ptr addrspace(1) %2, i16 6000, ptr @$string3, i16 1, i16 5999)
  %3 = call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %2, i16 6000)
  %4 = call cc 1005 addrspace(1) ptr @llrm.qb.B$FMID(ptr %3, i16 5999, i16 1)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %4)
  call void @llvm.lifetime.end.p0(i64 6000, ptr nonnull %1)
  ret void
}

define cc 1000 void @BIG() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [6000 x i8]
  %1 = addrspacecast ptr %0 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 6000, i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$SMID(ptr addrspace(1) %1, i16 6000, ptr @$string3, i16 1, i16 5999)
  %2 = call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %1, i16 6000)
  %3 = call cc 1005 addrspace(1) ptr @llrm.qb.B$FMID(ptr %2, i16 5999, i16 1)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %3)
  ret void
}

declare cc 1005 void @llrm.qb.B$SMID(ptr addrspace(1), i16, ptr, i16, i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
