target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"ROWCOUNT%" = internal global [2 x i8] zeroinitializer
@"COLUMNCOUNT%" = internal global [2 x i8] zeroinitializer
@"ITEMS%" = internal global [22 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"ROWCOUNT%" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"COLUMNCOUNT%" to ptr addrspace(1)))
  %0 = load i16, ptr @"ROWCOUNT%", !tbaa !2
  %1 = load i16, ptr @"COLUMNCOUNT%", !tbaa !2
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -2, i16 %0, i16 4, i16 %1, i16 2, i16 258, ptr @"ITEMS%")
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 18), !tbaa !2
  %3 = mul i16 %2, 10
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 10), !tbaa !2
  %6 = inttoptr i16 %4 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 %5
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 %3
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 -2
  store i16 123, ptr addrspace(1) %10, !tbaa !6
  %11 = shl i16 %2, 3
  %12 = getelementptr i8, ptr addrspace(1) %8, i16 %11
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 6
  store i16 456, ptr addrspace(1) %13, !tbaa !6
  %14 = load i16, ptr addrspace(1) %10, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %14)
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 18), !tbaa !2
  %16 = shl i16 %15, 3
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"ITEMS%", i16 10), !tbaa !2
  %19 = inttoptr i16 %17 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 %18
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 %16
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 6
  %24 = load i16, ptr addrspace(1) %23, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %24)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
