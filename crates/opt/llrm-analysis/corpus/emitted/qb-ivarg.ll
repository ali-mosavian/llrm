target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BRANCHCHOICE&" = internal global [4 x i8] zeroinitializer
@$qb$readData = internal constant [3 x i8] c" 1\00"
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"BRANCHCHOICE&" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %0)
  call cc 1000 addrspace(1) void @CHOOSEFIELD(ptr @"BRANCHCHOICE&")
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

define cc 1000 void @CHOOSEFIELD(ptr dereferenceable(4) %0) addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  store i16 0, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i16 7, ptr %3, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  store i16 10, ptr %2, !tbaa !2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %7 = load i16, ptr %1, !tbaa !2
  %8 = icmp sge i16 %7, 0
  br i1 %8, label %b3, label %b4

b3:
  %9 = load i16, ptr %4, !tbaa !2
  %10 = load i16, ptr %2, !tbaa !2
  %11 = icmp sle i16 %9, %10
  br i1 %11, label %b5, label %b6

b4:
  %12 = load i16, ptr %4, !tbaa !2
  %13 = load i16, ptr %2, !tbaa !2
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %b5, label %b6

b5:
  %15 = load i32, ptr %0
  %16 = icmp ne i32 %15, 0
  br i1 %16, label %b7, label %b8

b6:
  %17 = load i16, ptr %6, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %17)
  %18 = load i16, ptr %5, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %18)
  %19 = load i16, ptr %4, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %19)
  %20 = load i16, ptr %3, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %20)
  ret void

b7:
  %21 = load i16, ptr %3, !tbaa !2
  store i16 %21, ptr %6, !tbaa !2
  br label %b9

b8:
  %22 = load i16, ptr %3, !tbaa !2
  store i16 %22, ptr %5, !tbaa !2
  br label %b9

b9:
  %23 = load i16, ptr %3, !tbaa !2
  %24 = add i16 %23, 3
  store i16 %24, ptr %3, !tbaa !2
  %25 = load i16, ptr %4, !tbaa !2
  %26 = load i16, ptr %1, !tbaa !2
  %27 = add nsw i16 %25, %26
  store i16 %27, ptr %4, !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
