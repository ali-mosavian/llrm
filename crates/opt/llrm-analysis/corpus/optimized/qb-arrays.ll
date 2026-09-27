target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X&" = internal global [20 x i8] zeroinitializer
@X$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"X&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"X&", [6 x i8] c"\04\00\05\00\00\00" }>
@"Y&" = internal global [20 x i8] zeroinitializer
@Y$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"Y&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"Y&", [6 x i8] c"\04\00\05\00\00\00" }>
@"P&" = internal global [4 x i8] zeroinitializer
@"S&" = internal global [4 x i8] zeroinitializer
@"F&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@TAG$ = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string5$payload to ptr addrspace(2))
@$string5$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 4) to i16), [4 x i8] c"\01\00P\00" }>
@$string5$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string5$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [4 x i8] c"\01\00S\00" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [4 x i8] c"\01\00F\00" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [4 x i8] c"\01\00R\00" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i16 0, ptr @"I%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = load i16, ptr @"I%", !tbaa !2
  %2 = icmp sle i16 %1, 3
  br i1 %2, label %b5, label %b6

b5:
  %3 = add i16 %1, 1
  %4 = sext i16 %3 to i32
  %5 = mul i32 %4, 100000
  %6 = getelementptr inbounds i32, ptr @"X&", i16 %1
  store i32 %5, ptr %6, !tbaa !2
  %7 = add i16 %1, 2
  %8 = sext i16 %7 to i32
  %9 = mul i32 %8, 100000
  %10 = getelementptr inbounds i32, ptr @"Y&", i16 %1
  store i32 %9, ptr %10, !tbaa !2
  store i16 %3, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 0, ptr @"I%", !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 4
  store i16 3, ptr %11, !tbaa !2
  %12 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %12, !tbaa !2
  br label %b7

b7:
  %13 = load i16, ptr %12, !tbaa !2
  %14 = icmp sge i16 %13, 0
  br i1 %14, label %b8, label %b9

b8:
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = load i16, ptr %11, !tbaa !2
  %17 = icmp sle i16 %15, %16
  br i1 %17, label %b10, label %b11

b9:
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = load i16, ptr %11, !tbaa !2
  %20 = icmp sge i16 %18, %19
  br i1 %20, label %b10, label %b11

b10:
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = getelementptr inbounds i32, ptr @"X&", i16 %21
  %23 = load i32, ptr %22, !tbaa !2
  %24 = getelementptr inbounds i32, ptr @"Y&", i16 %21
  %25 = load i32, ptr %24, !tbaa !2
  %26 = mul i32 %23, %25
  store i32 %26, ptr @"P&", !tbaa !2
  %27 = sdiv i32 %26, 1000
  %28 = add i32 %27, 1000000
  store i32 %28, ptr @"S&", !tbaa !2
  %29 = sdiv i32 %28, 1000
  %30 = add i32 %29, 1
  %31 = sdiv i32 50000, %30
  store i32 %31, ptr @"F&", !tbaa !2
  %32 = load i32, ptr %22, !tbaa !2
  %33 = mul i32 %32, %31
  %34 = sdiv i32 %33, 512
  store i32 %34, ptr @"R&", !tbaa !2
  %35 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %21)
  %36 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %35)
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr %36, ptr @TAG$)
  %37 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string5$descriptor, ptr @TAG$)
  %38 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %37, ptr @$string8$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %38)
  %39 = load i32, ptr @"P&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %39)
  %40 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string10$descriptor, ptr @TAG$)
  %41 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %40, ptr @$string12$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %41)
  %42 = load i32, ptr @"S&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %42)
  %43 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string14$descriptor, ptr @TAG$)
  %44 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %43, ptr @$string16$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %44)
  %45 = load i32, ptr @"F&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %45)
  %46 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string18$descriptor, ptr @TAG$)
  %47 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %46, ptr @$string20$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %47)
  %48 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %48)
  %49 = load i16, ptr @"I%", !tbaa !2
  %50 = load i16, ptr %12, !tbaa !2
  %51 = add i16 %49, %50
  store i16 %51, ptr @"I%", !tbaa !2
  br label %b7

b11:
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string22$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
