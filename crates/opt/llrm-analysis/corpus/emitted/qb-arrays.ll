target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X&" = internal global [20 x i8] zeroinitializer, align 2
@X$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"X&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"X&", [6 x i8] c"\04\00\05\00\00\00" }>
@"Y&" = internal global [20 x i8] zeroinitializer, align 2
@Y$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"Y&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"Y&", [6 x i8] c"\04\00\05\00\00\00" }>
@"P&" = internal global [4 x i8] zeroinitializer
@"S&" = internal global [4 x i8] zeroinitializer
@"F&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
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

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 0, ptr @"I%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"I%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load i16, ptr @"I%", !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = add i16 %11, 1
  %13 = sext i16 %12 to i32
  %14 = mul i32 %13, 100000
  %15 = sub i16 %10, 0
  %16 = getelementptr inbounds i32, ptr @"X&", i16 %15
  store i32 %14, ptr %16, !tbaa !2
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = add i16 %18, 2
  %20 = sext i16 %19 to i32
  %21 = mul i32 %20, 100000
  %22 = sub i16 %17, 0
  %23 = getelementptr inbounds i32, ptr @"Y&", i16 %22
  store i32 %21, ptr %23, !tbaa !2
  %24 = load i16, ptr @"I%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 2
  %26 = load i16, ptr %25, !tbaa !2
  %27 = add nsw i16 %24, %26
  store i16 %27, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 0, ptr @"I%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 4
  store i16 3, ptr %28, !tbaa !2
  %29 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %29, !tbaa !2
  br label %b7

b7:
  %30 = getelementptr i8, ptr @$data, i16 6
  %31 = load i16, ptr %30, !tbaa !2
  %32 = icmp sge i16 %31, 0
  br i1 %32, label %b8, label %b9

b8:
  %33 = load i16, ptr @"I%", !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 4
  %35 = load i16, ptr %34, !tbaa !2
  %36 = icmp sle i16 %33, %35
  br i1 %36, label %b10, label %b11

b9:
  %37 = load i16, ptr @"I%", !tbaa !2
  %38 = getelementptr i8, ptr @$data, i16 4
  %39 = load i16, ptr %38, !tbaa !2
  %40 = icmp sge i16 %37, %39
  br i1 %40, label %b10, label %b11

b10:
  %41 = load i16, ptr @"I%", !tbaa !2
  %42 = sub i16 %41, 0
  %43 = getelementptr inbounds i32, ptr @"X&", i16 %42
  %44 = load i32, ptr %43, !tbaa !2
  %45 = load i16, ptr @"I%", !tbaa !2
  %46 = sub i16 %45, 0
  %47 = getelementptr inbounds i32, ptr @"Y&", i16 %46
  %48 = load i32, ptr %47, !tbaa !2
  %49 = mul i32 %44, %48
  store i32 %49, ptr @"P&", !tbaa !2
  %50 = load i32, ptr @"P&", !tbaa !2
  %51 = sdiv i32 %50, 1000
  %52 = add i32 %51, 1000000
  store i32 %52, ptr @"S&", !tbaa !2
  %53 = load i32, ptr @"S&", !tbaa !2
  %54 = sdiv i32 %53, 1000
  %55 = add i32 %54, 1
  %56 = icmp eq i32 %55, 0
  br i1 %56, label %b12, label %b13

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string13)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b12:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b13:
  %57 = sdiv i32 50000, %55
  store i32 %57, ptr @"F&", !tbaa !2
  %58 = load i16, ptr @"I%", !tbaa !2
  %59 = sub i16 %58, 0
  %60 = getelementptr inbounds i32, ptr @"X&", i16 %59
  %61 = load i32, ptr %60, !tbaa !2
  %62 = load i32, ptr @"F&", !tbaa !2
  %63 = mul i32 %61, %62
  %64 = sdiv i32 %63, 512
  store i32 %64, ptr @"R&", !tbaa !2
  %65 = load i16, ptr @"I%", !tbaa !2
  %66 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %65)
  %67 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %66)
  call cc 1000 addrspace(1) void @llrm.qb.B$SASS(ptr %67, ptr @TAG$)
  %68 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string5, ptr @TAG$)
  %69 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %68, ptr @$string6)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %69)
  %70 = load i32, ptr @"P&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %70)
  %71 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7, ptr @TAG$)
  %72 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %71, ptr @$string8)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %72)
  %73 = load i32, ptr @"S&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %73)
  %74 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string9, ptr @TAG$)
  %75 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %74, ptr @$string10)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %75)
  %76 = load i32, ptr @"F&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %76)
  %77 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string11, ptr @TAG$)
  %78 = call cc 1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %77, ptr @$string12)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %78)
  %79 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %79)
  %80 = load i16, ptr @"I%", !tbaa !2
  %81 = getelementptr i8, ptr @$data, i16 6
  %82 = load i16, ptr %81, !tbaa !2
  %83 = add nsw i16 %80, %82
  store i16 %83, ptr @"I%", !tbaa !2
  br label %b7
}

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
