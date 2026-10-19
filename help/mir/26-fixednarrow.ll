target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

@"ROWCOUNT%" = internal global [2 x i8] zeroinitializer
@"COLUMNCOUNT%" = internal global [2 x i8] zeroinitializer
@"ITEMS%" = internal global [22 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noreturn norecurse {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"ROWCOUNT%" to ptr addrspace(1)))
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"COLUMNCOUNT%" to ptr addrspace(1)))
  %0 = load i16, ptr @"ROWCOUNT%", !tbaa !2
  %1 = load i16, ptr @"COLUMNCOUNT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 -2, i16 %0, i16 4, i16 %1, i16 2, i16 258, ptr @"ITEMS%")
  %2 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 18), !tbaa !2
  %3 = mul i16 5, %2
  %4 = add i16 %3, -1
  %5 = mul i16 %4, 2
  %6 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %7 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 10), !tbaa !2
  %8 = add i16 %7, %5
  %9 = inttoptr i16 %6 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %8
  store i16 123, ptr addrspace(1) %11, !tbaa !6
  %12 = mul i16 4, %2
  %13 = add i16 %12, 3
  %14 = mul i16 %13, 2
  %15 = add i16 %7, %14
  %16 = getelementptr i8, ptr addrspace(1) %10, i16 %15
  store i16 456, ptr addrspace(1) %16, !tbaa !6
  %17 = load i16, ptr addrspace(1) %11, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %17)
  %18 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 18), !tbaa !2
  %19 = mul i16 4, %18
  %20 = add i16 %19, 3
  %21 = mul i16 %20, 2
  %22 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 2), !tbaa !2
  %23 = load i16, ptr getelementptr inbounds (i8, ptr @"ITEMS%", i16 10), !tbaa !2
  %24 = add i16 %23, %21
  %25 = inttoptr i16 %22 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 %24
  %28 = load i16, ptr addrspace(1) %27, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %28)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
