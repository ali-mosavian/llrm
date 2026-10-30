target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

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
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"R=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"A%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"B%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  %2 = addrspacecast ptr @"C%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %2)
  %3 = addrspacecast ptr @"D%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %3)
  %4 = addrspacecast ptr @"E%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %4)
  %5 = addrspacecast ptr @"F%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %5)
  %6 = addrspacecast ptr @"G%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %6)
  %7 = addrspacecast ptr @"H%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %7)
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
  br i1 %11, label %b3, label %b4

b3:
  %12 = load i16, ptr @"I%", !tbaa !2
  %13 = load i16, ptr @$data, !tbaa !2
  %14 = icmp sle i16 %12, %13
  br i1 %14, label %b5, label %b6

b4:
  %15 = load i16, ptr @"I%", !tbaa !2
  %16 = load i16, ptr @$data, !tbaa !2
  %17 = icmp sge i16 %15, %16
  br i1 %17, label %b5, label %b6

b5:
  %18 = load i16, ptr @"R%", !tbaa !2
  %19 = load i16, ptr @"A%", !tbaa !2
  %20 = load i16, ptr @"B%", !tbaa !2
  %21 = mul i16 %19, %20
  %22 = add i16 %18, %21
  %23 = load i16, ptr @"C%", !tbaa !2
  %24 = load i16, ptr @"D%", !tbaa !2
  %25 = mul i16 %23, %24
  %26 = add i16 %22, %25
  %27 = load i16, ptr @"E%", !tbaa !2
  %28 = load i16, ptr @"F%", !tbaa !2
  %29 = mul i16 %27, %28
  %30 = add i16 %26, %29
  %31 = load i16, ptr @"G%", !tbaa !2
  %32 = load i16, ptr @"H%", !tbaa !2
  %33 = mul i16 %31, %32
  %34 = add i16 %30, %33
  store i16 %34, ptr @"R%", !tbaa !2
  %35 = load i16, ptr @"I%", !tbaa !2
  %36 = getelementptr i8, ptr @$data, i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add nsw i16 %35, %37
  store i16 %38, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %39 = load i16, ptr @"R%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %39)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
