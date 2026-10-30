target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"V&" = internal global [4 x i8] zeroinitializer
@"S&" = internal global [4 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [8 x i8] c" 100000\00"
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"S=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"V&" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %0)
  store i32 0, ptr @"S&", !tbaa !2
  store i16 1, ptr @"I%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = getelementptr i8, ptr @$data, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = icmp sge i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = load i16, ptr @"I%", !tbaa !2
  %6 = load i16, ptr @$data, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr @"I%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  %11 = load i32, ptr @"S&", !tbaa !2
  %12 = load i32, ptr @"V&", !tbaa !2
  %13 = sdiv i32 %12, 7
  %14 = add i32 %11, %13
  %15 = load i32, ptr @"V&", !tbaa !2
  %16 = srem i32 %15, 7
  %17 = add i32 %14, %16
  store i32 %17, ptr @"S&", !tbaa !2
  %18 = load i16, ptr @"I%", !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 2
  %20 = load i16, ptr %19, !tbaa !2
  %21 = add nsw i16 %18, %20
  store i16 %21, ptr @"I%", !tbaa !2
  br label %b2

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %22 = load i32, ptr @"S&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %22)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
