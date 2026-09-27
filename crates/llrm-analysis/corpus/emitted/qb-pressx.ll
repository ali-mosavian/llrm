target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"A%" = internal global [2 x i8] zeroinitializer
@"B%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"D%" = internal global [2 x i8] zeroinitializer
@"E%" = internal global [2 x i8] zeroinitializer
@"F%" = internal global [2 x i8] zeroinitializer
@"G%" = internal global [2 x i8] zeroinitializer
@"H%" = internal global [2 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [29 x i8] c" 3, 5, 7, 11, 13, 17, 19, 23\00"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00R=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"A%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"B%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  %2 = addrspacecast ptr @"C%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %2)
  %3 = addrspacecast ptr @"D%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %3)
  %4 = addrspacecast ptr @"E%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %4)
  %5 = addrspacecast ptr @"F%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %5)
  %6 = addrspacecast ptr @"G%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %6)
  %7 = addrspacecast ptr @"H%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %7)
  store i16 0, ptr @"R%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %8 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %8, !tbaa !2
  br label %b2

b2:
  %9 = getelementptr i8, ptr @$data, i16 2
  %10 = load i16, ptr %9, !tbaa !2
  %11 = icmp sge i16 %10, 0
  %12 = sext i1 %11 to i16
  %13 = icmp ne i16 %12, 0
  br i1 %13, label %b3, label %b4

b3:
  %14 = load i16, ptr @"I%", !tbaa !2
  %15 = load i16, ptr @$data, !tbaa !2
  %16 = icmp sle i16 %14, %15
  %17 = sext i1 %16 to i16
  %18 = icmp ne i16 %17, 0
  br i1 %18, label %b5, label %b6

b4:
  %19 = load i16, ptr @"I%", !tbaa !2
  %20 = load i16, ptr @$data, !tbaa !2
  %21 = icmp sge i16 %19, %20
  %22 = sext i1 %21 to i16
  %23 = icmp ne i16 %22, 0
  br i1 %23, label %b5, label %b6

b5:
  %24 = load i16, ptr @"R%", !tbaa !2
  %25 = load i16, ptr @"A%", !tbaa !2
  %26 = load i16, ptr @"B%", !tbaa !2
  %27 = mul i16 %25, %26
  %28 = add i16 %24, %27
  %29 = load i16, ptr @"C%", !tbaa !2
  %30 = load i16, ptr @"D%", !tbaa !2
  %31 = mul i16 %29, %30
  %32 = add i16 %28, %31
  %33 = load i16, ptr @"E%", !tbaa !2
  %34 = load i16, ptr @"F%", !tbaa !2
  %35 = mul i16 %33, %34
  %36 = add i16 %32, %35
  %37 = load i16, ptr @"G%", !tbaa !2
  %38 = load i16, ptr @"H%", !tbaa !2
  %39 = mul i16 %37, %38
  %40 = add i16 %36, %39
  store i16 %40, ptr @"R%", !tbaa !2
  %41 = load i16, ptr @"I%", !tbaa !2
  %42 = getelementptr i8, ptr @$data, i16 2
  %43 = load i16, ptr %42, !tbaa !2
  %44 = add i16 %41, %43
  store i16 %44, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %45 = load i16, ptr @"R%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %45)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void
}

declare cc1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
