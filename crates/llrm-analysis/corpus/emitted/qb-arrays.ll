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
  %0 = sub i16 4, 1
  store i16 %0, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = getelementptr i8, ptr @$data, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = icmp sge i16 %3, 0
  %5 = sext i1 %4 to i16
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %b3, label %b4

b3:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sle i16 %7, %8
  %10 = sext i1 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr @"I%", !tbaa !2
  %13 = load i16, ptr @$data, !tbaa !2
  %14 = icmp sge i16 %12, %13
  %15 = sext i1 %14 to i16
  %16 = icmp ne i16 %15, 0
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = add i16 %18, 1
  %20 = sext i16 %19 to i32
  %21 = mul i32 %20, 100000
  %22 = sub i16 %17, 0
  %23 = getelementptr inbounds i32, ptr @"X&", i16 %22
  store i32 %21, ptr %23, !tbaa !2
  %24 = load i16, ptr @"I%", !tbaa !2
  %25 = load i16, ptr @"I%", !tbaa !2
  %26 = add i16 %25, 2
  %27 = sext i16 %26 to i32
  %28 = mul i32 %27, 100000
  %29 = sub i16 %24, 0
  %30 = getelementptr inbounds i32, ptr @"Y&", i16 %29
  store i32 %28, ptr %30, !tbaa !2
  %31 = load i16, ptr @"I%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add i16 %31, %33
  store i16 %34, ptr @"I%", !tbaa !2
  br label %b2

b6:
  store i16 0, ptr @"I%", !tbaa !2
  %35 = sub i16 4, 1
  %36 = getelementptr i8, ptr @$data, i16 4
  store i16 %35, ptr %36, !tbaa !2
  %37 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %37, !tbaa !2
  br label %b7

b7:
  %38 = getelementptr i8, ptr @$data, i16 6
  %39 = load i16, ptr %38, !tbaa !2
  %40 = icmp sge i16 %39, 0
  %41 = sext i1 %40 to i16
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b8, label %b9

b8:
  %43 = load i16, ptr @"I%", !tbaa !2
  %44 = getelementptr i8, ptr @$data, i16 4
  %45 = load i16, ptr %44, !tbaa !2
  %46 = icmp sle i16 %43, %45
  %47 = sext i1 %46 to i16
  %48 = icmp ne i16 %47, 0
  br i1 %48, label %b10, label %b11

b9:
  %49 = load i16, ptr @"I%", !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 4
  %51 = load i16, ptr %50, !tbaa !2
  %52 = icmp sge i16 %49, %51
  %53 = sext i1 %52 to i16
  %54 = icmp ne i16 %53, 0
  br i1 %54, label %b10, label %b11

b10:
  %55 = load i16, ptr @"I%", !tbaa !2
  %56 = sub i16 %55, 0
  %57 = getelementptr inbounds i32, ptr @"X&", i16 %56
  %58 = load i32, ptr %57, !tbaa !2
  %59 = load i16, ptr @"I%", !tbaa !2
  %60 = sub i16 %59, 0
  %61 = getelementptr inbounds i32, ptr @"Y&", i16 %60
  %62 = load i32, ptr %61, !tbaa !2
  %63 = mul i32 %58, %62
  store i32 %63, ptr @"P&", !tbaa !2
  %64 = load i32, ptr @"P&", !tbaa !2
  %65 = sdiv i32 %64, 1000
  %66 = add i32 %65, 1000000
  store i32 %66, ptr @"S&", !tbaa !2
  %67 = load i32, ptr @"S&", !tbaa !2
  %68 = sdiv i32 %67, 1000
  %69 = add i32 %68, 1
  %70 = sdiv i32 50000, %69
  store i32 %70, ptr @"F&", !tbaa !2
  %71 = load i16, ptr @"I%", !tbaa !2
  %72 = sub i16 %71, 0
  %73 = getelementptr inbounds i32, ptr @"X&", i16 %72
  %74 = load i32, ptr %73, !tbaa !2
  %75 = load i32, ptr @"F&", !tbaa !2
  %76 = mul i32 %74, %75
  %77 = sdiv i32 %76, 512
  store i32 %77, ptr @"R&", !tbaa !2
  %78 = load i16, ptr @"I%", !tbaa !2
  %79 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %78)
  %80 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %79)
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr %80, ptr @TAG$)
  %81 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string5$descriptor, ptr @TAG$)
  %82 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %81, ptr @$string8$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %82)
  %83 = load i32, ptr @"P&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %83)
  %84 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string10$descriptor, ptr @TAG$)
  %85 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %84, ptr @$string12$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %85)
  %86 = load i32, ptr @"S&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %86)
  %87 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string14$descriptor, ptr @TAG$)
  %88 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %87, ptr @$string16$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %88)
  %89 = load i32, ptr @"F&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %89)
  %90 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string18$descriptor, ptr @TAG$)
  %91 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %90, ptr @$string20$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %91)
  %92 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %92)
  %93 = load i16, ptr @"I%", !tbaa !2
  %94 = getelementptr i8, ptr @$data, i16 6
  %95 = load i16, ptr %94, !tbaa !2
  %96 = add i16 %93, %95
  store i16 %96, ptr @"I%", !tbaa !2
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
