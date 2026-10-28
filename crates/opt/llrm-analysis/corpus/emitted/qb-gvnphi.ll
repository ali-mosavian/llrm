target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"INPUTVALUE&" = internal global [4 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@"TRIAL%" = internal global [2 x i8] zeroinitializer
@"ANSWER&" = internal global [4 x i8] zeroinitializer
@"SQUARE&" = internal global [4 x i8] zeroinitializer
@$qb$readData = internal constant [12 x i8] c" 5, 0, 5, 1\00"
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  store i16 1, ptr @"TRIAL%", !tbaa !2
  store i16 2, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"TRIAL%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"TRIAL%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = addrspacecast ptr @"INPUTVALUE&" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %10)
  %11 = addrspacecast ptr @"BRANCHCHOICE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %11)
  %12 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %13 = icmp ne i16 %12, 0
  br i1 %13, label %b7, label %b8

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void

b7:
  %14 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %15 = add i32 %14, 1
  store i32 %15, ptr @"INPUTVALUE&", !tbaa !2
  %16 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %17 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %18 = mul i32 %16, %17
  %19 = add i32 %18, 1
  store i32 %19, ptr @"ANSWER&", !tbaa !2
  br label %b9

b8:
  %20 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %21 = add i32 %20, 2
  store i32 %21, ptr @"INPUTVALUE&", !tbaa !2
  %22 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %23 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %24 = mul i32 %22, %23
  %25 = sub i32 %24, 1
  store i32 %25, ptr @"ANSWER&", !tbaa !2
  br label %b9

b9:
  %26 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %27 = load i32, ptr @"INPUTVALUE&", !tbaa !2
  %28 = mul i32 %26, %27
  store i32 %28, ptr @"SQUARE&", !tbaa !2
  %29 = load i32, ptr @"ANSWER&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI4(i32 %29)
  %30 = load i32, ptr @"SQUARE&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %30)
  %31 = load i16, ptr @"TRIAL%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 2
  %33 = load i16, ptr %32, !tbaa !2
  %34 = add nsw i16 %31, %33
  store i16 %34, ptr @"TRIAL%", !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
