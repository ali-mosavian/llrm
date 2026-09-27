target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@"S%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [6 x i8] c" 7, 3\00"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00S=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"N%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"K%" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  store i16 0, ptr @"S%", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 20, ptr @$data, !tbaa !2
  %2 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %2, !tbaa !2
  br label %b2

b2:
  %3 = getelementptr i8, ptr @$data, i16 2
  %4 = load i16, ptr %3, !tbaa !2
  %5 = icmp sge i16 %4, 0
  %6 = sext i1 %5 to i16
  %7 = icmp ne i16 %6, 0
  br i1 %7, label %b3, label %b4

b3:
  %8 = load i16, ptr @"I%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sle i16 %8, %9
  %11 = sext i1 %10 to i16
  %12 = icmp ne i16 %11, 0
  br i1 %12, label %b5, label %b6

b4:
  %13 = load i16, ptr @"I%", !tbaa !2
  %14 = load i16, ptr @$data, !tbaa !2
  %15 = icmp sge i16 %13, %14
  %16 = sext i1 %15 to i16
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %b5, label %b6

b5:
  %18 = load i16, ptr @"S%", !tbaa !2
  %19 = load i16, ptr @"N%", !tbaa !2
  %20 = load i16, ptr @"K%", !tbaa !2
  %21 = mul i16 %19, %20
  %22 = add i16 %18, %21
  %23 = load i16, ptr @"I%", !tbaa !2
  %24 = add i16 %22, %23
  store i16 %24, ptr @"S%", !tbaa !2
  %25 = load i16, ptr @"I%", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = add i16 %25, %27
  store i16 %28, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %29 = load i16, ptr @"S%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %29)
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
