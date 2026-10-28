target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = global [0 x i8] zeroinitializer
@"A%" = global [8 x i8] zeroinitializer, align 2
@"X%" = global [2 x i8] zeroinitializer
@"L&" = global [4 x i8] zeroinitializer
@S$ = global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"hello\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"|\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"|\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  store i16 5, ptr @"X%", !tbaa !2
  store i32 70000, ptr @"L&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr @S$)
  store i16 9, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 4), !tbaa !2
  %0 = load i16, ptr @"X%", !tbaa !2
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %1)
  %2 = load i32, ptr @"L&", !tbaa !2
  %3 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %3)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @S$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 4), !tbaa !2
  %5 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SCLR(i16 0, i16 0)
  %6 = load i16, ptr @"X%", !tbaa !2
  %7 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %7)
  %8 = load i32, ptr @"L&", !tbaa !2
  %9 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %9)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @S$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 4), !tbaa !2
  %11 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %11)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$SCLR(i16, i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
