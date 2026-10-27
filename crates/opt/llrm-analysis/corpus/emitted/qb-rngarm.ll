target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"VALUES%" = internal global [8 x i8] zeroinitializer, align 2
@VALUES$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VALUES%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VALUES%", [6 x i8] c"\02\00\04\00\00\00" }>
@"INDEX%" = internal global [2 x i8] zeroinitializer
@"FACTOR%" = internal global [2 x i8] zeroinitializer
@"TOTAL%" = internal global [2 x i8] zeroinitializer
@$qb$readData = internal constant [3 x i8] c" 7\00"
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"FACTOR%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  store i16 0, ptr @"INDEX%", !tbaa !2
  store i16 9, ptr @$data, !tbaa !2
  %1 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %1, !tbaa !2
  br label %b2

b2:
  %2 = getelementptr i8, ptr @$data, i16 2
  %3 = load i16, ptr %2, !tbaa !2
  %4 = icmp sge i16 %3, 0
  br i1 %4, label %b3, label %b4

b3:
  %5 = load i16, ptr @"INDEX%", !tbaa !2
  %6 = load i16, ptr @$data, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr @"INDEX%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  %11 = load i16, ptr @"INDEX%", !tbaa !2
  %12 = icmp slt i16 %11, 4
  br i1 %12, label %b7, label %b8

b6:
  %13 = load i16, ptr @"TOTAL%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %13)
  %14 = getelementptr inbounds i16, ptr @"VALUES%", i16 0
  %15 = load i16, ptr %14, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %15)
  %16 = getelementptr inbounds i16, ptr @"VALUES%", i16 3
  %17 = load i16, ptr %16, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %17)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void

b7:
  %18 = load i16, ptr @"INDEX%", !tbaa !2
  %19 = load i16, ptr @"FACTOR%", !tbaa !2
  %20 = load i16, ptr @"INDEX%", !tbaa !2
  %21 = add i16 %19, %20
  %22 = sub i16 %18, 0
  %23 = getelementptr inbounds i16, ptr @"VALUES%", i16 %22
  store i16 %21, ptr %23, !tbaa !2
  %24 = load i16, ptr @"TOTAL%", !tbaa !2
  %25 = load i16, ptr @"FACTOR%", !tbaa !2
  %26 = add i16 %24, %25
  store i16 %26, ptr @"TOTAL%", !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %27 = load i16, ptr @"INDEX%", !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 2
  %29 = load i16, ptr %28, !tbaa !2
  %30 = add nsw i16 %27, %29
  store i16 %30, ptr @"INDEX%", !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
