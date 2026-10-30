target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@"SAMPLE%" = internal global [2 x i8] zeroinitializer
@"FIRSTVALUE#" = internal global [8 x i8] zeroinitializer
@"SECONDVALUE#" = internal global [8 x i8] zeroinitializer
@$qb$readData = internal constant [20 x i8] c" -32768, 123, 32766\00"
@$arguments$7 = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"VALUE=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 1, ptr @"SAMPLE%", !tbaa !2
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
  %4 = load i16, ptr @"SAMPLE%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"SAMPLE%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = addrspacecast ptr @"INPUTVALUE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %10)
  %11 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %12 = add i16 %11, 1
  store i16 %12, ptr @$arguments$7, !tbaa !2
  %13 = load i16, ptr @$arguments$7, !tbaa !2
  %14 = sitofp i16 %13 to double
  %15 = fpround double %14 to double
  store double %15, ptr @"FIRSTVALUE#", !tbaa !2
  %16 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %17 = add i16 %16, 1
  %18 = getelementptr i8, ptr @$arguments$7, i16 2
  store i16 %17, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$arguments$7, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = sitofp i16 %20 to double
  %22 = fpround double %21 to double
  store double %22, ptr @"SECONDVALUE#", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %23 = load double, ptr @"FIRSTVALUE#", !tbaa !2
  %24 = call i32 @llvm.lrint.i32.f64(double %23)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %24)
  %25 = load double, ptr @"SECONDVALUE#", !tbaa !2
  %26 = call i32 @llvm.lrint.i32.f64(double %25)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %26)
  %27 = load i16, ptr @"SAMPLE%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = add nsw i16 %27, %29
  store i16 %30, ptr @"SAMPLE%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
