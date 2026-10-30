target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A%" = internal global [18 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 100, i16 2, i16 257, ptr @"A%")
  %0 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), !tbaa !2
  %1 = inttoptr i16 %0 to ptr addrspace(2)
  %2 = addrspacecast ptr addrspace(2) %1 to ptr addrspace(1)
  %3 = getelementptr i8, ptr addrspace(1) %2, i16 2
  %4 = getelementptr i8, ptr addrspace(1) %2, i16 4
  %5 = getelementptr i8, ptr addrspace(1) %2, i16 6
  %6 = getelementptr i8, ptr addrspace(1) %2, i16 8
  %7 = getelementptr i8, ptr addrspace(1) %2, i16 10
  %8 = getelementptr i8, ptr addrspace(1) %2, i16 12
  %9 = getelementptr i8, ptr addrspace(1) %2, i16 14
  %10 = getelementptr i8, ptr addrspace(1) %2, i16 16
  %11 = getelementptr i8, ptr addrspace(1) %2, i16 18
  %12 = getelementptr i8, ptr addrspace(1) %2, i16 20
  %13 = getelementptr i8, ptr addrspace(1) %2, i16 22
  %14 = getelementptr i8, ptr addrspace(1) %2, i16 24
  %15 = getelementptr i8, ptr addrspace(1) %2, i16 26
  %16 = getelementptr i8, ptr addrspace(1) %2, i16 28
  %17 = getelementptr i8, ptr addrspace(1) %2, i16 30
  %18 = getelementptr i8, ptr addrspace(1) %2, i16 32
  %19 = getelementptr i8, ptr addrspace(1) %2, i16 34
  %20 = getelementptr i8, ptr addrspace(1) %2, i16 36
  %21 = getelementptr i8, ptr addrspace(1) %2, i16 38
  %22 = getelementptr i8, ptr addrspace(1) %2, i16 40
  store i16 1, ptr addrspace(1) %3, !tbaa !6
  store i16 2, ptr addrspace(1) %4, !tbaa !6
  store i16 3, ptr addrspace(1) %5, !tbaa !6
  store i16 4, ptr addrspace(1) %6, !tbaa !6
  store i16 5, ptr addrspace(1) %7, !tbaa !6
  store i16 6, ptr addrspace(1) %8, !tbaa !6
  store i16 7, ptr addrspace(1) %9, !tbaa !6
  store i16 8, ptr addrspace(1) %10, !tbaa !6
  store i16 9, ptr addrspace(1) %11, !tbaa !6
  store i16 10, ptr addrspace(1) %12, !tbaa !6
  store i16 11, ptr addrspace(1) %13, !tbaa !6
  store i16 12, ptr addrspace(1) %14, !tbaa !6
  store i16 13, ptr addrspace(1) %15, !tbaa !6
  store i16 14, ptr addrspace(1) %16, !tbaa !6
  store i16 15, ptr addrspace(1) %17, !tbaa !6
  store i16 16, ptr addrspace(1) %18, !tbaa !6
  store i16 17, ptr addrspace(1) %19, !tbaa !6
  store i16 18, ptr addrspace(1) %20, !tbaa !6
  store i16 19, ptr addrspace(1) %21, !tbaa !6
  store i16 20, ptr addrspace(1) %22, !tbaa !6
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 1050)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
