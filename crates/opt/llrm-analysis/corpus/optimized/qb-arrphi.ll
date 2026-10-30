target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@"FIRSTVALUES%" = internal global [18 x i8] zeroinitializer
@"SECONDVALUES%" = internal global [18 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 4, i16 2, i16 257, ptr @"FIRSTVALUES%")
  %0 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %1 = icmp eq i16 %0, 0
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FIRSTVALUES%", i16 2), !tbaa !2
  %3 = inttoptr i16 %2 to ptr addrspace(2)
  %4 = addrspacecast ptr addrspace(2) %3 to ptr addrspace(1)
  %5 = getelementptr i8, ptr addrspace(1) %4, i16 2
  %6 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %7 = select i1 %1, i16 2, i16 1
  %8 = add i16 %6, %7
  store i16 %8, ptr addrspace(1) %5, !tbaa !6
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"FIRSTVALUES%", i16 2), !tbaa !2
  %10 = inttoptr i16 %9 to ptr addrspace(2)
  %11 = addrspacecast ptr addrspace(2) %10 to ptr addrspace(1)
  %12 = getelementptr i8, ptr addrspace(1) %11, i16 2
  %13 = load i16, ptr addrspace(1) %12, !tbaa !6
  %14 = add i16 %13, 3
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE%" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  tail call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 4, i16 2, i16 257, ptr @"SECONDVALUES%")
  %15 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %16 = icmp eq i16 %15, 0
  %17 = select i1 %16, i16 2, i16 1
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"SECONDVALUES%", i16 2), !tbaa !2
  %19 = inttoptr i16 %18 to ptr addrspace(2)
  %20 = addrspacecast ptr addrspace(2) %19 to ptr addrspace(1)
  %21 = getelementptr i8, ptr addrspace(1) %20, i16 2
  %22 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %23 = add i16 %22, %17
  store i16 %23, ptr addrspace(1) %21, !tbaa !8
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"SECONDVALUES%", i16 2), !tbaa !2
  %25 = inttoptr i16 %24 to ptr addrspace(2)
  %26 = addrspacecast ptr addrspace(2) %25 to ptr addrspace(1)
  %27 = getelementptr i8, ptr addrspace(1) %26, i16 2
  %28 = load i16, ptr addrspace(1) %27, !tbaa !8
  %29 = add i16 %28, 3
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %29)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
