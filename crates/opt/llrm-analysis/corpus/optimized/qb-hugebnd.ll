target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"H&" = internal global [46 x i8] zeroinitializer
@"G%" = internal global [46 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 0, i16 32766, i16 4, i16 257, ptr @"H&")
  tail call cc 1000 addrspace(1) void @llrm.qb.B$RDIM(i16 -2, i16 0, i16 -2, i16 32764, i16 2, i16 258, ptr @"G%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"H&", i16 2), !tbaa !2
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"H&", i16 10), !tbaa !2
  %2 = inttoptr i16 %0 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = getelementptr i8, ptr addrspace(1) %3, i16 %1
  store i32 5, ptr addrspace(1) %4, !tbaa !6
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 -8
  store i32 7, ptr addrspace(1) %5, !tbaa !6
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 18), !tbaa !2
  %7 = mul i16 %6, -4
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %10 = inttoptr i16 %8 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 %9
  %13 = getelementptr i8, ptr addrspace(1) %12, i16 %7
  %14 = getelementptr i8, ptr addrspace(1) %13, i16 -4
  store i16 11, ptr addrspace(1) %14, !tbaa !8
  %15 = mul i16 %6, -8
  %16 = getelementptr i8, ptr addrspace(1) %12, i16 %15
  store i16 13, ptr addrspace(1) %16, !tbaa !8
  %17 = mul i16 %6, -25536
  %18 = getelementptr i8, ptr addrspace(1) %12, i16 %17
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 -2
  store i16 17, ptr addrspace(1) %19, !tbaa !8
  %20 = load i32, ptr addrspace(1) %4, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %20)
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"H&", i16 2), !tbaa !2
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"H&", i16 10), !tbaa !2
  %23 = inttoptr i16 %21 to ptr addrspace(2)
  %24 = addrspacecast ptr addrspace(2) %23 to ptr addrspace(1)
  %25 = getelementptr i8, ptr addrspace(1) %24, i16 %22
  %26 = getelementptr i8, ptr addrspace(1) %25, i16 -8
  %27 = load i32, ptr addrspace(1) %26, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %27)
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 18), !tbaa !2
  %29 = mul i16 %28, -4
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %31 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %32 = inttoptr i16 %30 to ptr addrspace(2)
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 %31
  %35 = getelementptr i8, ptr addrspace(1) %34, i16 %29
  %36 = getelementptr i8, ptr addrspace(1) %35, i16 -4
  %37 = load i16, ptr addrspace(1) %36, !tbaa !8
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %37)
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 18), !tbaa !2
  %39 = mul i16 %38, -8
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %42 = inttoptr i16 %40 to ptr addrspace(2)
  %43 = addrspacecast ptr addrspace(2) %42 to ptr addrspace(1)
  %44 = getelementptr i8, ptr addrspace(1) %43, i16 %41
  %45 = getelementptr i8, ptr addrspace(1) %44, i16 %39
  %46 = load i16, ptr addrspace(1) %45, !tbaa !8
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %46)
  %47 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 18), !tbaa !2
  %48 = mul i16 %47, -25536
  %49 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 2), !tbaa !2
  %50 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"G%", i16 10), !tbaa !2
  %51 = inttoptr i16 %49 to ptr addrspace(2)
  %52 = addrspacecast ptr addrspace(2) %51 to ptr addrspace(1)
  %53 = getelementptr i8, ptr addrspace(1) %52, i16 %50
  %54 = getelementptr i8, ptr addrspace(1) %53, i16 %48
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 -2
  %56 = load i16, ptr addrspace(1) %55, !tbaa !8
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %56)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 -2)
  ret void
}

declare cc 1000 void @llrm.qb.B$RDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
