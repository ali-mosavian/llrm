target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@A$ = internal global [4 x i8] zeroinitializer
@B$ = internal global [4 x i8] zeroinitializer
@"N&" = internal global [4 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [16 x i8] }> <{ [2 x i8] c"\10\00", ptr getelementptr (i8, ptr @$string3, i16 4), [16 x i8] c"Hello, Far World" }>
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"!\00" }>
@"P%" = internal global [2 x i8] zeroinitializer
@b$seg = external global [2 x i8]
@"I%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c"|\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"|\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr @$string3, ptr @A$)
  %0 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @A$, ptr @$string4)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %0, ptr @B$)
  %1 = call cc 1000 addrspace(1) i16 @llrm.qb.B$SADD(ptr @A$)
  store i16 %1, ptr @"P%", !tbaa !2
  %2 = addrspacecast ptr @A$ to ptr addrspace(1)
  %3 = addrspacecast ptr addrspace(1) %2 to ptr addrspace(2)
  %4 = ptrtoint ptr addrspace(2) %3 to i16
  store i16 %4, ptr @b$seg, !tbaa !2
  store i16 0, ptr @"I%", !tbaa !2
  %5 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr @A$)
  %6 = sub i16 %5, 1
  store i16 %6, ptr @$data, !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %7, !tbaa !2
  br label %b2

b2:
  %8 = getelementptr i8, ptr @$data, i16 2
  %9 = load i16, ptr %8, !tbaa !2
  %10 = icmp sge i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sle i16 %11, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr @"I%", !tbaa !2
  %15 = load i16, ptr @$data, !tbaa !2
  %16 = icmp sge i16 %14, %15
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr @"P%", !tbaa !2
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = add i16 %17, %18
  %20 = load i16, ptr @b$seg, !tbaa !2
  %21 = inttoptr i16 %20 to ptr addrspace(2)
  %22 = addrspacecast ptr addrspace(2) %21 to ptr addrspace(1)
  %23 = getelementptr i8, ptr addrspace(1) %22, i16 %19
  %24 = load volatile i8, ptr addrspace(1) %23
  %25 = zext i8 %24 to i16
  store i16 %25, ptr @"C%", !tbaa !2
  %26 = load i32, ptr @"N&", !tbaa !2
  %27 = load i16, ptr @"C%", !tbaa !2
  %28 = sext i16 %27 to i32
  %29 = add i32 %26, %28
  store i32 %29, ptr @"N&", !tbaa !2
  %30 = load i16, ptr @"C%", !tbaa !2
  %31 = icmp sge i16 %30, 65
  %32 = sext i1 %31 to i16
  %33 = load i16, ptr @"C%", !tbaa !2
  %34 = icmp sle i16 %33, 90
  %35 = sext i1 %34 to i16
  %36 = and i16 %32, %35
  %37 = icmp ne i16 %36, 0
  br i1 %37, label %b7, label %b8

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$DSG0()
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @A$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @B$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %38 = load i32, ptr @"N&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %38)
  %39 = call cc 1000 addrspace(1) i16 @llrm.qb.B$FLEN(ptr @A$)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  ret void

b7:
  %40 = load i16, ptr @"P%", !tbaa !2
  %41 = load i16, ptr @"I%", !tbaa !2
  %42 = add i16 %40, %41
  %43 = load i16, ptr @"C%", !tbaa !2
  %44 = add i16 %43, 32
  %45 = trunc i16 %44 to i8
  %46 = load i16, ptr @b$seg, !tbaa !2
  %47 = inttoptr i16 %46 to ptr addrspace(2)
  %48 = addrspacecast ptr addrspace(2) %47 to ptr addrspace(1)
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 %42
  store volatile i8 %45, ptr addrspace(1) %49
  br label %b9

b8:
  %50 = load i16, ptr @"C%", !tbaa !2
  %51 = icmp sge i16 %50, 97
  %52 = sext i1 %51 to i16
  %53 = load i16, ptr @"C%", !tbaa !2
  %54 = icmp sle i16 %53, 122
  %55 = sext i1 %54 to i16
  %56 = and i16 %52, %55
  %57 = icmp ne i16 %56, 0
  br i1 %57, label %b10, label %b11

b9:
  %58 = load i16, ptr @"I%", !tbaa !2
  %59 = getelementptr i8, ptr @$data, i16 2
  %60 = load i16, ptr %59, !tbaa !2
  %61 = add nsw i16 %58, %60
  store i16 %61, ptr @"I%", !tbaa !2
  br label %b2

b10:
  %62 = load i16, ptr @"P%", !tbaa !2
  %63 = load i16, ptr @"I%", !tbaa !2
  %64 = add i16 %62, %63
  %65 = load i16, ptr @"C%", !tbaa !2
  %66 = sub i16 %65, 32
  %67 = trunc i16 %66 to i8
  %68 = load i16, ptr @b$seg, !tbaa !2
  %69 = inttoptr i16 %68 to ptr addrspace(2)
  %70 = addrspacecast ptr addrspace(2) %69 to ptr addrspace(1)
  %71 = getelementptr i8, ptr addrspace(1) %70, i16 %64
  store volatile i8 %67, ptr addrspace(1) %71
  br label %b12

b11:
  br label %b12

b12:
  br label %b9
}

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$SADD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FLEN(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$DSG0() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
