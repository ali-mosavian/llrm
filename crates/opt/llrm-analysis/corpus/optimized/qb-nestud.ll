target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@O = internal global [6 x i8] zeroinitializer
@ARR = internal global [12 x i8] zeroinitializer, align 2
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"AB" }>
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"CD" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"EF" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"DONE" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"GH" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"IJ" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"KL" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  store i32 111, ptr @O, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string4, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @O, i16 4) to ptr addrspace(1)), i16 2)
  store i32 1, ptr @ARR, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string5, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 4) to ptr addrspace(1)), i16 2)
  store i32 2, ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 6), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string6, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 10) to ptr addrspace(1)), i16 2)
  %0 = load i32, ptr @O, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %0)
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @O, i16 4) to ptr addrspace(1)), i16 2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %1)
  %2 = load i32, ptr @ARR, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %2)
  %3 = load i32, ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 6), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  %4 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 4) to ptr addrspace(1)), i16 2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %4)
  %5 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @ARR, i16 10) to ptr addrspace(1)), i16 2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %5)
  tail call cc 1000 addrspace(1) void @INSIDE()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 999)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
  ret void
}

define cc 1000 void @INSIDE() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  %0 = alloca [24 x i8]
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 4
  %2 = getelementptr inbounds nuw i8, ptr %0, i16 6
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 8
  call void @llvm.memset.p0.i16(ptr align 2 %0, i8 0, i16 24, i1 false)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 1, i16 6, i16 257, ptr nonnull %2)
  store i32 333, ptr %0, !tbaa !2
  %4 = addrspacecast ptr %1 to ptr addrspace(1)
  call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string8, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %4, i16 2)
  %5 = load i16, ptr %3, !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  store i32 4, ptr addrspace(1) %7, !tbaa !6
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 4
  call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string9, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %8, i16 2)
  %9 = load i16, ptr %3, !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 6
  store i32 5, ptr addrspace(1) %12, !tbaa !6
  %13 = getelementptr i8, ptr addrspace(1) %11, i16 10
  call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string10, i16 4) to ptr addrspace(1)), i16 2, ptr addrspace(1) nocapture writeonly initializes((0, 2)) %13, i16 2)
  %14 = load i32, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  %15 = call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %4, i16 2)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %15)
  %16 = load i16, ptr %3, !tbaa !2
  %17 = inttoptr i16 %16 to ptr addrspace(2)
  %18 = addrspacecast ptr addrspace(2) %17 to ptr addrspace(1)
  %19 = load i32, ptr addrspace(1) %18, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %19)
  %20 = load i16, ptr %3, !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 6
  %24 = load i32, ptr addrspace(1) %23, !tbaa !6
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %24)
  %25 = load i16, ptr %3, !tbaa !2
  %26 = inttoptr i16 %25 to ptr addrspace(2)
  %27 = addrspacecast ptr addrspace(2) %26 to ptr addrspace(1)
  %28 = getelementptr i8, ptr addrspace(1) %27, i16 4
  %29 = call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %28, i16 2)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %29)
  %30 = load i16, ptr %3, !tbaa !2
  %31 = inttoptr i16 %30 to ptr addrspace(2)
  %32 = addrspacecast ptr addrspace(2) %31 to ptr addrspace(1)
  %33 = getelementptr i8, ptr addrspace(1) %32, i16 10
  %34 = call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) %33, i16 2)
  call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %34)
  call cc 1005 addrspace(1) void @llrm.qb.B$ERAS(ptr nonnull %2)
  ret void
}

declare cc 1005 void @llrm.qb.B$ASSN(ptr addrspace(1), i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$ERAS(ptr nocapture noretain) addrspace(1) nocallback

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
