target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X&" = internal global [20 x i8] zeroinitializer, align 2
@"Y&" = internal global [20 x i8] zeroinitializer, align 2
@TAG$ = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"P\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"S\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"F\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"=\00" }>
@$string11 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string11, i16 4), [2 x i8] c"R\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string12, i16 4), [2 x i8] c"=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  store i32 100000, ptr @"X&", !tbaa !2
  store i32 200000, ptr @"Y&", !tbaa !2
  store i32 200000, ptr getelementptr inbounds nuw (i8, ptr @"X&", i16 4), !tbaa !2
  store i32 300000, ptr getelementptr inbounds nuw (i8, ptr @"Y&", i16 4), !tbaa !2
  store i32 300000, ptr getelementptr inbounds nuw (i8, ptr @"X&", i16 8), !tbaa !2
  store i32 400000, ptr getelementptr inbounds nuw (i8, ptr @"Y&", i16 8), !tbaa !2
  store i32 400000, ptr getelementptr inbounds nuw (i8, ptr @"X&", i16 12), !tbaa !2
  store i32 500000, ptr getelementptr inbounds nuw (i8, ptr @"Y&", i16 12), !tbaa !2
  br label %b10

b10:
  %0 = phi i16 [ %29, %b13 ], [ -4, %b1 ]
  %1 = phi i16 [ %28, %b13 ], [ -16, %b1 ]
  %2 = add i16 %0, 4
  %3 = getelementptr i8, ptr @"X&", i16 %1
  %4 = getelementptr i8, ptr %3, i16 16
  %5 = load i32, ptr %4, !tbaa !2
  %6 = getelementptr i8, ptr @"Y&", i16 %1
  %7 = getelementptr i8, ptr %6, i16 16
  %8 = load i32, ptr %7, !tbaa !2
  %9 = mul i32 %8, %5
  %10 = sdiv i32 %9, 1000
  %11 = add nsw i32 %10, 1000000
  %12 = sdiv i32 %11, 1000
  %13 = add nsw i32 %12, 1
  %14 = icmp eq i32 %13, 0
  br i1 %14, label %b12, label %b13

b11:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b12:
  tail call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b13:
  %15 = sdiv i32 50000, %13
  %16 = mul i32 %15, %5
  %17 = sdiv i32 %16, 512
  %18 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI2(i16 %2)
  %19 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %18)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %19, ptr @TAG$)
  %20 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string5, ptr @TAG$)
  %21 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %20, ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %21)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %9)
  %22 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7, ptr @TAG$)
  %23 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %22, ptr @$string8)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %23)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  %24 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string9, ptr @TAG$)
  %25 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %24, ptr @$string10)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %25)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  %26 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string11, ptr @TAG$)
  %27 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %26, ptr @$string12)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %27)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %17)
  %28 = add nsw i16 %1, 4
  %29 = add nsw i16 %0, 1
  %exitcond = icmp eq i16 %28, 0
  br i1 %exitcond, label %b11, label %b10
}

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) nocallback noreturn

declare cc 1005 ptr @llrm.qb.B$STI2(i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
