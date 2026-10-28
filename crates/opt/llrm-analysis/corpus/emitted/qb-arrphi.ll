target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"INPUTVALUE%" = internal global [2 x i8] zeroinitializer
@"BRANCHCHOICE%" = internal global [2 x i8] zeroinitializer
@"ANSWER%" = internal global [2 x i8] zeroinitializer
@"FIRSTVALUES%" = internal global [18 x i8] zeroinitializer
@"SECONDVALUES%" = internal global [18 x i8] zeroinitializer
@$qb$readData = internal constant [12 x i8] c" 5, 0, 5, 1\00"
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
  %0 = addrspacecast ptr @"INPUTVALUE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %0)
  %1 = addrspacecast ptr @"BRANCHCHOICE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 4, i16 2, i16 257, ptr @"FIRSTVALUES%")
  %2 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %3 = icmp ne i16 %2, 0
  br i1 %3, label %b2, label %b3

b2:
  %4 = getelementptr i8, ptr @"FIRSTVALUES%", i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = inttoptr i16 %5 to ptr addrspace(2)
  %7 = addrspacecast ptr addrspace(2) %6 to ptr addrspace(1)
  %8 = getelementptr i8, ptr addrspace(1) %7, i16 2
  %9 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %10 = add i16 %9, 1
  store i16 %10, ptr addrspace(1) %8, !tbaa !6
  br label %b4

b3:
  %11 = getelementptr i8, ptr @"FIRSTVALUES%", i16 2
  %12 = load i16, ptr %11, !tbaa !2
  %13 = inttoptr i16 %12 to ptr addrspace(2)
  %14 = addrspacecast ptr addrspace(2) %13 to ptr addrspace(1)
  %15 = getelementptr i8, ptr addrspace(1) %14, i16 2
  %16 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %17 = add i16 %16, 2
  store i16 %17, ptr addrspace(1) %15, !tbaa !6
  br label %b4

b4:
  %18 = getelementptr i8, ptr @"FIRSTVALUES%", i16 2
  %19 = load i16, ptr %18, !tbaa !2
  %20 = inttoptr i16 %19 to ptr addrspace(2)
  %21 = addrspacecast ptr addrspace(2) %20 to ptr addrspace(1)
  %22 = getelementptr i8, ptr addrspace(1) %21, i16 2
  %23 = load i16, ptr addrspace(1) %22, !tbaa !6
  %24 = add i16 %23, 3
  store i16 %24, ptr @"ANSWER%", !tbaa !2
  %25 = load i16, ptr @"ANSWER%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %25)
  %26 = addrspacecast ptr @"INPUTVALUE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %26)
  %27 = addrspacecast ptr @"BRANCHCHOICE%" to ptr addrspace(1)
  call cc 1000 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) %27)
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 1, i16 4, i16 2, i16 257, ptr @"SECONDVALUES%")
  %28 = load i16, ptr @"BRANCHCHOICE%", !tbaa !2
  %29 = icmp ne i16 %28, 0
  br i1 %29, label %b5, label %b6

b5:
  %30 = getelementptr i8, ptr @"SECONDVALUES%", i16 2
  %31 = load i16, ptr %30, !tbaa !2
  %32 = inttoptr i16 %31 to ptr addrspace(2)
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 2
  %35 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %36 = add i16 %35, 1
  store i16 %36, ptr addrspace(1) %34, !tbaa !8
  br label %b7

b6:
  %37 = getelementptr i8, ptr @"SECONDVALUES%", i16 2
  %38 = load i16, ptr %37, !tbaa !2
  %39 = inttoptr i16 %38 to ptr addrspace(2)
  %40 = addrspacecast ptr addrspace(2) %39 to ptr addrspace(1)
  %41 = getelementptr i8, ptr addrspace(1) %40, i16 2
  %42 = load i16, ptr @"INPUTVALUE%", !tbaa !2
  %43 = add i16 %42, 2
  store i16 %43, ptr addrspace(1) %41, !tbaa !8
  br label %b7

b7:
  %44 = getelementptr i8, ptr @"SECONDVALUES%", i16 2
  %45 = load i16, ptr %44, !tbaa !2
  %46 = inttoptr i16 %45 to ptr addrspace(2)
  %47 = addrspacecast ptr addrspace(2) %46 to ptr addrspace(1)
  %48 = getelementptr i8, ptr addrspace(1) %47, i16 2
  %49 = load i16, ptr addrspace(1) %48, !tbaa !8
  %50 = add i16 %49, 3
  store i16 %50, ptr @"ANSWER%", !tbaa !2
  %51 = load i16, ptr @"ANSWER%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %51)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void
}

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @llrm.qb.B$RDI2(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
!7 = !{!"allocation.1", !3, i64 0}
!8 = !{!7, !7, i64 0}
