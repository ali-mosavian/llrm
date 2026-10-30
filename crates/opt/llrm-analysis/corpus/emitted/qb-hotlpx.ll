target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"N%" = internal global [2 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@"S%" = internal global [2 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [6 x i8] c" 7, 3\00"
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"S=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"N%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"K%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
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
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"I%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  br i1 %8, label %b5, label %b6

b4:
  %9 = load i16, ptr @"I%", !tbaa !2
  %10 = load i16, ptr @$data, !tbaa !2
  %11 = icmp sge i16 %9, %10
  br i1 %11, label %b5, label %b6

b5:
  %12 = load i16, ptr @"S%", !tbaa !2
  %13 = load i16, ptr @"N%", !tbaa !2
  %14 = load i16, ptr @"K%", !tbaa !2
  %15 = mul i16 %13, %14
  %16 = add i16 %12, %15
  %17 = load i16, ptr @"I%", !tbaa !2
  %18 = add i16 %16, %17
  store i16 %18, ptr @"S%", !tbaa !2
  %19 = load i16, ptr @"I%", !tbaa !2
  %20 = getelementptr i8, ptr @$data, i16 2
  %21 = load i16, ptr %20, !tbaa !2
  %22 = add nsw i16 %19, %21
  store i16 %22, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %23 = load i16, ptr @"S%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %23)
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
