target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@"ANSWER%" = internal global [2 x i8] zeroinitializer
@"FIRSTVALUES%" = internal global [8 x i8] zeroinitializer, align 2
@FIRSTVALUES$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"FIRSTVALUES%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"FIRSTVALUES%", i16 -2), [6 x i8] c"\02\00\04\00\01\00" }>
@"SECONDVALUES%" = internal global [8 x i8] zeroinitializer, align 2
@SECONDVALUES$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"SECONDVALUES%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"SECONDVALUES%", i16 -2), [6 x i8] c"\02\00\04\00\01\00" }>
@$qb$readData = internal constant [12 x i8] c" 5, 0, 5, 1\00"
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"INPUTVALUE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"BRANCHCHOICE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  %2 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %3 = icmp ne i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %5 = add i16 %4, 1
  %6 = getelementptr inbounds i16, ptr @"FIRSTVALUES%", i16 1
  store i16 %5, ptr %6, !tbaa !2
  br label %b4

b3:
  %7 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %8 = add i16 %7, 2
  %9 = getelementptr inbounds i16, ptr @"FIRSTVALUES%", i16 1
  store i16 %8, ptr %9, !tbaa !2
  br label %b4

b4:
  %10 = getelementptr inbounds i16, ptr @"FIRSTVALUES%", i16 1
  %11 = load i16, ptr %10, !tbaa !2
  %12 = add i16 %11, 3
  store i16 %12, ptr @"ANSWER%", !tbaa !2
  %13 = load i16, ptr @"ANSWER%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %13)
  %14 = addrspacecast ptr @"INPUTVALUE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %14)
  %15 = addrspacecast ptr @"BRANCHCHOICE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %15)
  %16 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %17 = icmp ne i16 %16, 0
  br i1 %17, label %b5, label %b6

b5:
  %18 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %19 = add i16 %18, 1
  %20 = getelementptr inbounds i16, ptr @"SECONDVALUES%", i16 1
  store i16 %19, ptr %20, !tbaa !2
  br label %b7

b6:
  %21 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %22 = add i16 %21, 2
  %23 = getelementptr inbounds i16, ptr @"SECONDVALUES%", i16 1
  store i16 %22, ptr %23, !tbaa !2
  br label %b7

b7:
  %24 = getelementptr inbounds i16, ptr @"SECONDVALUES%", i16 1
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add i16 %25, 3
  store i16 %26, ptr @"ANSWER%", !tbaa !2
  %27 = load i16, ptr @"ANSWER%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
