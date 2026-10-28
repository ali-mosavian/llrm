target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@P$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string3, i16 4), [14 x i8] c"Oliver|Scrooge" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"SUM=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"LEN=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"MID=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"TEMP=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"x\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"ab" }>
@$string11 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string11, i16 4), [2 x i8] c"ab" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"ONE=" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"CHR=" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"x\00" }>
@E$.0 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
  %slot2 = alloca i8
  %view2 = alloca [4 x i8]
  %slot3 = alloca i8
  %view3 = alloca [4 x i8]
  %slot4 = alloca i8
  %view4 = alloca [4 x i8]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @P$)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %length17 = load i16, ptr @P$
  %1 = icmp slt i16 %length17, 1
  br i1 %1, label %12, label %2

2:
  %data11 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @P$, i16 2)
  br label %3

3:
  %4 = phi ptr [ %10, %3 ], [ %data11, %2 ]
  %5 = phi i16 [ %9, %3 ], [ %length17, %2 ]
  %6 = phi i32 [ 0, %2 ], [ %8, %3 ]
  %byte6 = load i8, ptr %4
  %7 = zext i8 %byte6 to i32
  %8 = add i32 %6, %7
  %9 = add i16 %5, -1
  %10 = getelementptr i8, ptr %4, i16 1
  %11 = icmp eq i16 %9, 0
  br i1 %11, label %12, label %3

12:
  %13 = phi i32 [ 0, %0 ], [ %8, %3 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %length7 = load i16, ptr @P$
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %length7)
  %length6 = load i16, ptr @E$.0
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %length6)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %length16 = load i16, ptr @P$
  %14 = tail call i16 @llvm.smax.i16(i16 %length16, i16 0)
  %15 = tail call i16 @llvm.umin.i16(i16 3, i16 %14)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %15)
  %length15 = load i16, ptr @P$
  %16 = add i16 %length15, -12
  %17 = tail call i16 @llvm.smax.i16(i16 %16, i16 0)
  %18 = tail call i16 @llvm.umin.i16(i16 5, i16 %17)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %18)
  %length14 = load i16, ptr @P$
  %19 = add i16 %length14, -13
  %20 = tail call i16 @llvm.smax.i16(i16 %19, i16 0)
  %21 = tail call i16 @llvm.umin.i16(i16 1, i16 %20)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %21)
  %length13 = load i16, ptr @P$
  %22 = add i16 %length13, -14
  %23 = tail call i16 @llvm.smax.i16(i16 %22, i16 0)
  %24 = tail call i16 @llvm.umin.i16(i16 1, i16 %23)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %24)
  %length12 = load i16, ptr @P$
  %25 = add i16 %length12, -98
  %26 = tail call i16 @llvm.smax.i16(i16 %25, i16 0)
  %27 = tail call i16 @llvm.umin.i16(i16 1, i16 %26)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %27)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %28 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LEFT(ptr @P$, i16 2)
  %29 = tail call cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %28)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %29)
  %30 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string9)
  %31 = tail call cc 1005 addrspace(1) i16 @llrm.qb.B$FASC(ptr %30)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %31)
  %32 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string10)
  %length5 = load i16, ptr %32
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %length5)
  %33 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string11)
  %34 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$FMID(ptr %33, i16 3, i16 4)
  %length4 = load i16, ptr %34
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %length4)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %data7 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @P$, i16 2)
  %35 = getelementptr i8, ptr %data7, i16 1
  %byte5 = load i8, ptr %35
  %36 = zext i8 %byte5 to i16
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %36)
  %data5 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @P$, i16 2)
  %37 = getelementptr i8, ptr %data5, i16 13
  %byte4 = load i8, ptr %37
  %38 = zext i8 %byte4 to i16
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %38)
  %data3 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @P$, i16 2)
  %byte3 = load i8, ptr %data3
  %39 = zext i8 %byte3 to i16
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 65)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 1)
  store i8 67, ptr %slot4
  store i16 1, ptr %view4
  %40 = getelementptr inbounds nuw i8, ptr %view4, i16 2
  store ptr %slot4, ptr %40
  store i8 68, ptr %slot3
  store i16 1, ptr %view3
  %41 = getelementptr inbounds nuw i8, ptr %view3, i16 2
  store ptr %slot3, ptr %41
  %42 = call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr nonnull %view4, ptr nonnull %view3)
  call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr %42)
  store i8 -56, ptr %slot2
  store i16 1, ptr %view2
  %43 = getelementptr inbounds nuw i8, ptr %view2, i16 2
  store ptr %slot2, ptr %43
  %44 = call cc 1005 addrspace(1) ptr @llrm.qb.B$SCAT(ptr nonnull %view2, ptr @$string14)
  %length = load i16, ptr %44
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %length)
  call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 255)
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 0)
  ret void
}

define cc 1000 i16 @"EDGE%"(ptr dereferenceable(4) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1, ptr dereferenceable(2) nocapture readonly %2) addrspace(1) "llrm-fp-eval"="x87-extended" willreturn memory(argmem: read) nounwind nofree nosync norecurse {
  %4 = load i16, ptr %1
  %5 = load i16, ptr %2
  %length1 = load i16, ptr %0
  %6 = sub i16 %length1, %4
  %7 = add i16 %6, 1
  %8 = tail call i16 @llvm.smax.i16(i16 %7, i16 0)
  %9 = tail call i16 @llvm.smin.i16(i16 %5, i16 %8)
  ret i16 %9
}

define cc 1000 i32 @"SUM&"(ptr dereferenceable(4) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, inaccessiblemem: none) willreturn nounwind nofree nosync norecurse {
  %length1 = load i16, ptr %0
  %2 = icmp slt i16 %length1, 1
  br i1 %2, label %b6, label %3

3:
  %4 = getelementptr inbounds nuw i8, ptr %0, i16 2
  %data1 = load ptr, ptr %4
  br label %5

5:
  %6 = phi ptr [ %12, %5 ], [ %data1, %3 ]
  %7 = phi i16 [ %11, %5 ], [ %length1, %3 ]
  %8 = phi i32 [ 0, %3 ], [ %10, %5 ]
  %byte = load i8, ptr %6
  %9 = zext i8 %byte to i32
  %10 = add i32 %8, %9
  %11 = add i16 %7, -1
  %12 = getelementptr i8, ptr %6, i16 1
  %13 = icmp eq i16 %11, 0
  br i1 %13, label %b6, label %5

b6:
  %14 = phi i32 [ 0, %1 ], [ %10, %5 ]
  ret i32 %14
}

declare cc 1005 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$FLEN(ptr) addrspace(1) "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LEFT(ptr, i16) addrspace(1) nocallback

declare cc 1005 i16 @llrm.qb.B$FASC(ptr releases) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$FMID(ptr releases, i16, i16) addrspace(1) alwaysinline "llrm-fp-eval"="x87-extended" nocallback willreturn

declare cc 1005 ptr @llrm.qb.B$FCHR(i16) addrspace(1) "llrm-fp-eval"="x87-extended" nocallback willreturn

declare i16 @llvm.smax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.umin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
