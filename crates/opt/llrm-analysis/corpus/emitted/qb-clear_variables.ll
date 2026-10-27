target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [8 x i8] zeroinitializer, align 2
@A$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"A%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"A%", [6 x i8] c"\02\00\04\00\00\00" }>
@"X%" = internal global [2 x i8] zeroinitializer
@"L&" = internal global [4 x i8] zeroinitializer
@S$ = internal global [4 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string4, i16 4), [6 x i8] c"hello\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"|\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"|\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 5, ptr @"X%", !tbaa !2
  store i32 70000, ptr @"L&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string4, ptr @S$)
  %0 = getelementptr inbounds i16, ptr @"A%", i16 2
  store i16 9, ptr %0, !tbaa !2
  %1 = load i16, ptr @"X%", !tbaa !2
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %2)
  %3 = load i32, ptr @"L&", !tbaa !2
  %4 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %3)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @S$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %5 = getelementptr inbounds i16, ptr @"A%", i16 2
  %6 = load i16, ptr %5, !tbaa !2
  %7 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %6)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %7)
  call cc 1000 addrspace(1) void @llrm.qb.B$SCLR(i16 0, i16 0)
  %8 = load i16, ptr @"X%", !tbaa !2
  %9 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %9)
  %10 = load i32, ptr @"L&", !tbaa !2
  %11 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %10)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %11)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @S$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %12 = getelementptr inbounds i16, ptr @"A%", i16 2
  %13 = load i16, ptr %12, !tbaa !2
  %14 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %13)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %14)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SCLR(i16, i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
