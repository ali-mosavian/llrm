target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@A$ = internal global [4 x i8] zeroinitializer
@B$ = internal global [4 x i8] zeroinitializer
@"N&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [16 x i8] }> <{ [2 x i8] c"\10\00", ptr getelementptr (i8, ptr @$string3, i16 4), [16 x i8] c"Hello, Far World" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"!\00" }>
@b$seg = external global [2 x i8]
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"|\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"|\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @A$)
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @A$, ptr @$string4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr %1, ptr @B$)
  %2 = tail call cc 1005 addrspace(1) i16 @llrm.qb.B$SADD(ptr @A$)
  store i16 ptrtoint (ptr addrspace(2) addrspacecast (ptr @A$ to ptr addrspace(2)) to i16), ptr @b$seg, !tbaa !2
  %length1 = load i16, ptr @A$
  %3 = add i16 %length1, -1
  %4 = icmp slt i16 %3, 0
  br i1 %4, label %19, label %5

5:
  br label %b5

b5:
  %6 = phi i16 [ %31, %b9 ], [ %2, %5 ]
  %7 = phi i16 [ %30, %b9 ], [ %length1, %5 ]
  %8 = phi i16 [ %29, %b9 ], [ ptrtoint (ptr addrspace(2) addrspacecast (ptr @A$ to ptr addrspace(2)) to i16), %5 ]
  %9 = inttoptr i16 %8 to ptr addrspace(2)
  %10 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %11 = getelementptr i8, ptr addrspace(1) %10, i16 %6
  %12 = load volatile i8, ptr addrspace(1) %11
  %13 = load i32, ptr @"N&", !tbaa !2
  %14 = zext i8 %12 to i32
  %15 = add i32 %13, %14
  store i32 %15, ptr @"N&", !tbaa !2
  %16 = icmp ugt i8 %12, 64
  %17 = icmp ult i8 %12, 91
  %18 = and i1 %16, %17
  br i1 %18, label %24, label %b8

19:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$DSG0()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @A$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @B$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %20 = load i32, ptr @"N&", !tbaa !2
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI4(i32 %20)
  %length = load i16, ptr @A$
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %length)
  ret void

b8:
  %21 = icmp ugt i8 %12, 96
  %22 = icmp ult i8 %12, 123
  %23 = and i1 %21, %22
  br i1 %23, label %24, label %b9

24:
  %sink1 = phi i8 [ -32, %b8 ], [ 32, %b5 ]
  %25 = addrspacecast ptr addrspace(2) %9 to ptr addrspace(1)
  %26 = add nsw i8 %sink1, %12
  %27 = getelementptr i8, ptr addrspace(1) %25, i16 %6
  store volatile i8 %26, ptr addrspace(1) %27
  %28 = load i16, ptr @b$seg, !tbaa !2
  br label %b9

b9:
  %29 = phi i16 [ %28, %24 ], [ %8, %b8 ]
  %30 = add i16 %7, -1
  %31 = add i16 %6, 1
  %exitcond = icmp eq i16 %30, 0
  br i1 %exitcond, label %19, label %b5
}

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$SADD(ptr) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$FLEN(ptr) addrspace(1) "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 void @llrm.qb.B$DSG0() addrspace(1) nocallback nounwind

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSI4(i32) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
