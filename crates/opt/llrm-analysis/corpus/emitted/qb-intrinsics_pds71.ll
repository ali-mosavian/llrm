target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@P$ = internal global [4 x i8] zeroinitializer
@E$ = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [14 x i8] }> <{ [2 x i8] c"\0E\00", ptr getelementptr (i8, ptr @$string3, i16 4), [14 x i8] c"Oliver|Scrooge" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"SUM=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"LEN=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"MID=" }>
@$arguments$11 = internal global [24 x i8] zeroinitializer
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"TEMP=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c"x\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string10, i16 4), [2 x i8] c"ab" }>
@$string11 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string11, i16 4), [2 x i8] c"ab" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"ONE=" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"CHR=" }>
@$string14 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string14, i16 4), [2 x i8] c"x\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @P$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %0 = call cc 1000 addrspace(1) i32 @"SUM&"(ptr @P$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %0)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %1 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr @P$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %1)
  %2 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr @E$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  store i16 1, ptr @$arguments$11, !tbaa !2
  %3 = getelementptr i8, ptr @$arguments$11, i16 2
  store i16 3, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$11, i16 2
  %5 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr @$arguments$11, ptr %4)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %5)
  %6 = getelementptr i8, ptr @$arguments$11, i16 4
  store i16 13, ptr %6, !tbaa !2
  %7 = getelementptr i8, ptr @$arguments$11, i16 4
  %8 = getelementptr i8, ptr @$arguments$11, i16 6
  store i16 5, ptr %8, !tbaa !2
  %9 = getelementptr i8, ptr @$arguments$11, i16 6
  %10 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr %7, ptr %9)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %10)
  %11 = getelementptr i8, ptr @$arguments$11, i16 8
  store i16 14, ptr %11, !tbaa !2
  %12 = getelementptr i8, ptr @$arguments$11, i16 8
  %13 = getelementptr i8, ptr @$arguments$11, i16 10
  store i16 1, ptr %13, !tbaa !2
  %14 = getelementptr i8, ptr @$arguments$11, i16 10
  %15 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr %12, ptr %14)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %15)
  %16 = getelementptr i8, ptr @$arguments$11, i16 12
  store i16 15, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$arguments$11, i16 12
  %18 = getelementptr i8, ptr @$arguments$11, i16 14
  store i16 1, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$arguments$11, i16 14
  %20 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr %17, ptr %19)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %20)
  %21 = getelementptr i8, ptr @$arguments$11, i16 16
  store i16 99, ptr %21, !tbaa !2
  %22 = getelementptr i8, ptr @$arguments$11, i16 16
  %23 = getelementptr i8, ptr @$arguments$11, i16 18
  store i16 1, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$arguments$11, i16 18
  %25 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr %22, ptr %24)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %25)
  %26 = getelementptr i8, ptr @$arguments$11, i16 20
  store i16 5, ptr %26, !tbaa !2
  %27 = getelementptr i8, ptr @$arguments$11, i16 20
  %28 = getelementptr i8, ptr @$arguments$11, i16 22
  store i16 0, ptr %28, !tbaa !2
  %29 = getelementptr i8, ptr @$arguments$11, i16 22
  %30 = call cc 1000 addrspace(1) i16 @"EDGE%"(ptr @P$, ptr %27, ptr %29)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %30)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %31 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LEFT(ptr @P$, i16 2)
  %32 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %31)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %32)
  %33 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string9)
  %34 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %33)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %34)
  %35 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string10)
  %36 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %35)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %36)
  %37 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @P$, ptr @$string11)
  %38 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %37, i16 3, i16 4)
  %39 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %38)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %40 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr @P$, i16 2, i16 1)
  %41 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %40)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %41)
  %42 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr @P$, i16 14, i16 9)
  %43 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %42)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %43)
  %44 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr @P$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %44)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  %45 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 65)
  %46 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %45)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %46)
  %47 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 66)
  %48 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %47)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %48)
  %49 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 67)
  %50 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 68)
  %51 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %49, ptr %50)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %51)
  %52 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 200)
  %53 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %52, ptr @$string14)
  %54 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %53)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %54)
  %55 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 255)
  %56 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %55)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %56)
  %57 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FCHR(i16 0)
  %58 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %57)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %58)
  ret void
}

define cc 1000 i16 @"EDGE%"(ptr dereferenceable(4) %0, ptr dereferenceable(2) %1, ptr dereferenceable(2) %2) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %3 = alloca i16
  %4 = load i16, ptr %1
  %5 = load i16, ptr %2
  %6 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %0, i16 %4, i16 %5)
  %7 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %6)
  store i16 %7, ptr %3, !tbaa !2
  br label %b2

b2:
  %8 = load i16, ptr %3, !tbaa !2
  ret i16 %8
}

define cc 1000 i32 @"SUM&"(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i16
  %5 = alloca i32
  store i32 0, ptr %3, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  %6 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr %0)
  store i16 %6, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %1, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = load i16, ptr %2, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i16, ptr %2, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i32, ptr %3, !tbaa !2
  %16 = load i16, ptr %4, !tbaa !2
  %17 = call cc 1000 addrspace(1) ptr @llrm.qb.B$FMID(ptr %0, i16 %16, i16 1)
  %18 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FASC(ptr %17)
  %19 = sext i16 %18 to i32
  %20 = add i32 %15, %19
  store i32 %20, ptr %3, !tbaa !2
  %21 = load i16, ptr %4, !tbaa !2
  %22 = load i16, ptr %1, !tbaa !2
  %23 = add nsw i16 %21, %22
  store i16 %23, ptr %4, !tbaa !2
  br label %b2

b6:
  %24 = load i32, ptr %3, !tbaa !2
  store i32 %24, ptr %5, !tbaa !2
  br label %b7

b7:
  %25 = load i32, ptr %5, !tbaa !2
  ret i32 %25
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FLEN(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LEFT(ptr, i16) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FASC(ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FMID(ptr, i16, i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$FCHR(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
