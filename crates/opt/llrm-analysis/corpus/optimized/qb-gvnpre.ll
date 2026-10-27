target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"INPUTVALUE&" = internal global [4 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %0 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %1 = add i32 %0, 1
  store i32 %1, ptr @"INPUTVALUE&", !tbaa !2
  %2 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %3 = icmp eq i16 %2, 0
  br i1 %3, label %b8, label %b7

b7:
  %4 = mul i32 %1, %1
  %5 = add i32 %4, 1
  br label %b9

b8:
  %6 = add i32 %0, 2
  %7 = mul i32 %1, %1
  br label %b9

b9:
  %8 = phi i32 [ %4, %b7 ], [ %7, %b8 ]
  %9 = phi i32 [ %5, %b7 ], [ %6, %b8 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) addrspacecast (ptr @"INPUTVALUE&" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"BRANCHCHOICE%" to ptr addrspace(1)))
  %10 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %11 = add i32 %10, 1
  store i32 %11, ptr @"INPUTVALUE&", !tbaa !2
  %12 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %13 = icmp eq i16 %12, 0
  br i1 %13, label %14, label %17

14:
  %15 = add i32 %10, 2
  %16 = mul i32 %11, %11
  br label %20

17:
  %18 = mul i32 %11, %11
  %19 = add i32 %18, 1
  br label %20

20:
  %21 = phi i32 [ %18, %17 ], [ %16, %14 ]
  %22 = phi i32 [ %19, %17 ], [ %15, %14 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %22)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %21)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI4(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
