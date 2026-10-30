target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"NUMS&" = internal global [8 x i8] zeroinitializer, align 2
@NUMS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"NUMS&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"NUMS&", [6 x i8] c"\04\00\02\00\00\00" }>
@PTS = internal global [16 x i8] zeroinitializer, align 2
@PTS$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @PTS to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @PTS, [6 x i8] c"\08\00\02\00\00\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @NUMS$descriptor, i16 2)
  %2 = inttoptr i16 %1 to ptr addrspace(2)
  %3 = addrspacecast ptr addrspace(2) %2 to ptr addrspace(1)
  %4 = getelementptr i8, ptr addrspace(1) %3, i16 ptrtoint (ptr @"NUMS&" to i16)
  store i32 7, ptr addrspace(1) %4
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 4
  store i32 8, ptr addrspace(1) %5
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @PTS$descriptor, i16 2)
  %7 = inttoptr i16 %6 to ptr addrspace(2)
  %8 = addrspacecast ptr addrspace(2) %7 to ptr addrspace(1)
  %9 = getelementptr i8, ptr addrspace(1) %8, i16 ptrtoint (ptr @PTS to i16)
  store i32 1, ptr addrspace(1) %9
  %10 = getelementptr inbounds nuw i8, ptr addrspace(1) %9, i16 4
  store i32 2, ptr addrspace(1) %10
  %11 = getelementptr i8, ptr addrspace(1) %9, i16 8
  store i32 3, ptr addrspace(1) %11
  %12 = getelementptr i8, ptr addrspace(1) %9, i16 12
  store i32 4, ptr addrspace(1) %12
  %13 = load i32, ptr @"NUMS&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %13)
  %14 = load i32, ptr getelementptr inbounds nuw (i8, ptr @"NUMS&", i16 4), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %14)
  %15 = load i32, ptr @PTS, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %15)
  %16 = load i32, ptr getelementptr inbounds nuw (i8, ptr @PTS, i16 4), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %16)
  %17 = load i32, ptr getelementptr inbounds nuw (i8, ptr @PTS, i16 8), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %17)
  %18 = load i32, ptr getelementptr inbounds nuw (i8, ptr @PTS, i16 12), !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %18)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

define cc 1000 void @FILLNUMS(ptr dereferenceable(18) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: readwrite, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = inttoptr i16 %2 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 %4
  store i32 7, ptr addrspace(1) %7
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 4
  store i32 8, ptr addrspace(1) %8
  ret void
}

define cc 1000 void @FILLPTS(ptr dereferenceable(18) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: readwrite, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
b1:
  %1 = getelementptr inbounds nuw i8, ptr %0, i16 2
  %2 = load i16, ptr %1
  %3 = getelementptr inbounds nuw i8, ptr %0, i16 10
  %4 = load i16, ptr %3
  %5 = inttoptr i16 %2 to ptr addrspace(2)
  %6 = addrspacecast ptr addrspace(2) %5 to ptr addrspace(1)
  %7 = getelementptr i8, ptr addrspace(1) %6, i16 %4
  store i32 1, ptr addrspace(1) %7
  %8 = getelementptr inbounds nuw i8, ptr addrspace(1) %7, i16 4
  store i32 2, ptr addrspace(1) %8
  %9 = getelementptr i8, ptr addrspace(1) %7, i16 8
  store i32 3, ptr addrspace(1) %9
  %10 = getelementptr i8, ptr addrspace(1) %7, i16 12
  store i32 4, ptr addrspace(1) %10
  ret void
}

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
