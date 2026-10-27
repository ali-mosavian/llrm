target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"BRANCHCHOICE&" = internal global [4 x i8] zeroinitializer
@"STEPCOUNT%" = internal global [2 x i8] zeroinitializer
@"CURRENTVALUE%" = internal global [2 x i8] zeroinitializer
@"VALUES%" = internal global [4 x i8] zeroinitializer, align 2
@VALUES$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VALUES%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VALUES%", [6 x i8] c"\02\00\02\00\00\00" }>
@$qb$readData = internal constant [3 x i8] c" 1\00"
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"BRANCHCHOICE&" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI4(ptr addrspace(1) %0)
  store i16 7, ptr @"CURRENTVALUE%", !tbaa !2
  store i16 1, ptr @"STEPCOUNT%", !tbaa !2
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
  %5 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %6 = load i16, ptr @$data, !tbaa !2
  %7 = icmp sle i16 %5, %6
  br i1 %7, label %b5, label %b6

b4:
  %8 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %9 = load i16, ptr @$data, !tbaa !2
  %10 = icmp sge i16 %8, %9
  br i1 %10, label %b5, label %b6

b5:
  %11 = load i32, ptr @"BRANCHCHOICE&", !tbaa !2
  %12 = icmp ne i32 %11, 0
  br i1 %12, label %b7, label %b8

b6:
  %13 = getelementptr inbounds i16, ptr @"VALUES%", i16 0
  %14 = load i16, ptr %13, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %14)
  %15 = getelementptr inbounds i16, ptr @"VALUES%", i16 1
  %16 = load i16, ptr %15, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %16)
  %17 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %17)
  %18 = load i16, ptr @"CURRENTVALUE%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %18)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void

b7:
  %19 = load i16, ptr @"CURRENTVALUE%", !tbaa !2
  %20 = getelementptr inbounds i16, ptr @"VALUES%", i16 0
  store i16 %19, ptr %20, !tbaa !2
  br label %b9

b8:
  %21 = load i16, ptr @"CURRENTVALUE%", !tbaa !2
  %22 = getelementptr inbounds i16, ptr @"VALUES%", i16 1
  store i16 %21, ptr %22, !tbaa !2
  br label %b9

b9:
  %23 = load i16, ptr @"CURRENTVALUE%", !tbaa !2
  %24 = add i16 %23, 3
  store i16 %24, ptr @"CURRENTVALUE%", !tbaa !2
  %25 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 2
  %27 = load i16, ptr %26, !tbaa !2
  %28 = add nsw i16 %25, %27
  store i16 %28, ptr @"STEPCOUNT%", !tbaa !2
  br label %b2
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
