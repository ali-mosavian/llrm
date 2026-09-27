target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"INPUTVALUE&" = internal global [4 x i8] zeroinitializer
@"SAMPLE%" = internal global [2 x i8] zeroinitializer
@"FIRSTVALUE#" = internal global [8 x i8] zeroinitializer
@"SECONDVALUE#" = internal global [8 x i8] zeroinitializer
@$qb$readData = internal constant [20 x i8] c" -32768, 123, 32767\00"
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [8 x i8] c"\06\00VALUE=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 1, ptr @"SAMPLE%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  %4 = sext i1 %3 to i16
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"SAMPLE%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr @"SAMPLE%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sge i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b5:
  %16 = addrspacecast ptr @"INPUTVALUE&" to ptr addrspace(1)
  call cc1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %16)
  %17 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 4
  store i32 %17, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 4
  %20 = load i32, ptr %19, !tbaa !2
  %21 = sitofp i32 %20 to double
  store double %21, ptr @"FIRSTVALUE#", !tbaa !2
  %22 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 8
  store i32 %22, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 8
  %25 = load i32, ptr %24, !tbaa !2
  %26 = sitofp i32 %25 to double
  store double %26, ptr @"SECONDVALUE#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %27 = load double, ptr @"FIRSTVALUE#", !tbaa !2
  %28 = call i32 @llvm.lrint.i32.f64(double %27)
  call cc1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %28)
  %29 = load double, ptr @"SECONDVALUE#", !tbaa !2
  %30 = call i32 @llvm.lrint.i32.f64(double %29)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %30)
  %31 = load i16, ptr @"SAMPLE%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add i16 %31, %33
  store i16 %34, ptr @"SAMPLE%", !tbaa !2
  br label %b2

b6:
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
