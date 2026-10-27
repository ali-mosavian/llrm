target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@R = internal global [10 x i8] zeroinitializer
@Q = internal global [10 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string3, i16 4), [12 x i8] c"OPENMODE.DAT" }>
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"seven\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"three\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string6, i16 4), [12 x i8] c"OPENMODE.DAT" }>
@$string7 = internal constant <{ [2 x i8], ptr, [12 x i8] }> <{ [2 x i8] c"\0C\00", ptr getelementptr (i8, ptr @$string7, i16 4), [12 x i8] c"OPENMODE.DAT" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string3, i16 1, i16 10, i16 772)
  store i16 7, ptr @R, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string4, i16 4) to ptr addrspace(1)), i16 5, ptr addrspace(1) nocapture writeonly initializes((0, 8)) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @R, i16 2) to ptr addrspace(1)), i16 8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PUT4(i16 1, i32 2, ptr addrspace(1) addrspacecast (ptr @R to ptr addrspace(1)), i16 10)
  store i16 3, ptr @R, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$ASSN(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$string5, i16 4) to ptr addrspace(1)), i16 5, ptr addrspace(1) nocapture writeonly initializes((0, 8)) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @R, i16 2) to ptr addrspace(1)), i16 8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PUT4(i16 1, i32 1, ptr addrspace(1) addrspacecast (ptr @R to ptr addrspace(1)), i16 10)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 1, i16 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string6, i16 2, i16 10, i16 4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$GET4(i16 2, i32 2, ptr addrspace(1) addrspacecast (ptr @Q to ptr addrspace(1)), i16 10)
  %0 = load i16, ptr @Q, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @Q, i16 2) to ptr addrspace(1)), i16 8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$GET4(i16 2, i32 1, ptr addrspace(1) addrspacecast (ptr @Q to ptr addrspace(1)), i16 10)
  %2 = load i16, ptr @Q, !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %2)
  %3 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LDFS(ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @Q, i16 2) to ptr addrspace(1)), i16 8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %3)
  %4 = tail call cc 1005 addrspace(1) i32 @llrm.qb.B$FLOF(i16 2)
  %5 = sdiv i32 %4, 10
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 2, i16 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$OPEN(ptr @$string7, i16 3, i16 -1, i16 257)
  %6 = tail call cc 1005 addrspace(1) i16 @llrm.qb.B$FEOF(i16 3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %6)
  tail call cc 1000 addrspace(1) void @llrm.qb.B$CLOS(i16 3, i16 1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$OPEN(ptr, i16, i16, i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$ASSN(ptr addrspace(1), i16, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PUT4(i16, i32, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1000 void @llrm.qb.B$CLOS(i16, i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$GET4(i16, i32, ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LDFS(ptr addrspace(1), i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 i32 @llrm.qb.B$FLOF(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$FEOF(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
