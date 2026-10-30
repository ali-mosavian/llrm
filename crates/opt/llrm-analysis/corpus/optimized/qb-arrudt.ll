target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  %1 = alloca [18 x i8]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 33)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 22)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 44)
  call void @llvm.lifetime.start.p0(i64 18, ptr nonnull %1)
  %2 = getelementptr inbounds nuw i8, ptr %1, i16 2
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %1, i8 0, i16 18, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 8, i16 257, ptr nonnull %1)
  %3 = load i16, ptr %2, !tbaa !2
  %4 = inttoptr i16 %3 to ptr addrspace(2)
  %5 = addrspacecast ptr addrspace(2) %4 to ptr addrspace(1)
  store i32 55, ptr addrspace(1) %5, !tbaa !6
  %6 = getelementptr inbounds nuw i8, ptr addrspace(1) %5, i16 4
  store i32 66, ptr addrspace(1) %6, !tbaa !6
  %7 = getelementptr i8, ptr addrspace(1) %5, i16 8
  store i32 77, ptr addrspace(1) %7, !tbaa !6
  %8 = getelementptr i8, ptr addrspace(1) %5, i16 12
  store i32 88, ptr addrspace(1) %8, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 55)
  %9 = load i16, ptr %2, !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 8
  %13 = load i32, ptr addrspace(1) %12, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  %14 = load i16, ptr %2, !tbaa !2
  %15 = inttoptr i16 %14 to ptr addrspace(2)
  %16 = addrspacecast ptr addrspace(2) %15 to ptr addrspace(1)
  %17 = getelementptr inbounds nuw i8, ptr addrspace(1) %16, i16 4
  %18 = load i32, ptr addrspace(1) %17, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %18)
  %19 = load i16, ptr %2, !tbaa !2
  %20 = inttoptr i16 %19 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 12
  %23 = load i32, ptr addrspace(1) %22, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %23)
  call cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %1)
  call void @llvm.lifetime.end.p0(i64 18, ptr nonnull %1)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

define cc 1000 void @INSIDE() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca [18 x i8]
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 18, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 8, i16 257, ptr nonnull %0)
  %2 = load i16, ptr %1, !tbaa !2
  %3 = inttoptr i16 %2 to ptr addrspace(2)
  %4 = addrspacecast ptr addrspace(2) %3 to ptr addrspace(1)
  store i32 55, ptr addrspace(1) %4, !tbaa !6
  %5 = getelementptr inbounds nuw i8, ptr addrspace(1) %4, i16 4
  store i32 66, ptr addrspace(1) %5, !tbaa !6
  %6 = getelementptr i8, ptr addrspace(1) %4, i16 8
  store i32 77, ptr addrspace(1) %6, !tbaa !6
  %7 = getelementptr i8, ptr addrspace(1) %4, i16 12
  store i32 88, ptr addrspace(1) %7, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 55)
  %8 = load i16, ptr %1, !tbaa !2
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 8
  %12 = load i32, ptr addrspace(1) %11, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %12)
  %13 = load i16, ptr %1, !tbaa !2
  %14 = inttoptr i16 %13 to ptr addrspace(2)
  %15 = addrspacecast ptr addrspace(2) %14 to ptr addrspace(1)
  %16 = getelementptr inbounds nuw i8, ptr addrspace(1) %15, i16 4
  %17 = load i32, ptr addrspace(1) %16, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %17)
  %18 = load i16, ptr %1, !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 12
  %22 = load i32, ptr addrspace(1) %21, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %22)
  call cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %0)
  ret void
}

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
