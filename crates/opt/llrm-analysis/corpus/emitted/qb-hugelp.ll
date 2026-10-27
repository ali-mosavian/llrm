target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"ITEMS%" = internal global [22 x i8] zeroinitializer
@"STEPCOUNT%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 200, i16 -2, i16 198, i16 2, i16 258, ptr @"ITEMS%")
  store i16 0, ptr @"STEPCOUNT%", !tbaa !2
  store i16 1, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  %10 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %11 = add i16 4, %10
  %12 = add i16 32361, %11
  %13 = mul i16 %12, 2
  %14 = getelementptr i8, ptr @"ITEMS%", i16 2
  %15 = load i16, ptr %14, !tbaa !2
  %16 = add i16 804, %13
  %17 = inttoptr i16 %15 to ptr addrspace(2)
  %18 = addrspacecast ptr addrspace(2) %17 to ptr addrspace(1)
  %19 = getelementptr i8, ptr addrspace(1) %18, i16 %16
  %20 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %21 = add i16 456, %20
  store i16 %21, ptr addrspace(1) %19, !tbaa !6
  %22 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %23 = add i16 2, %22
  %24 = mul i16 %23, 201
  %25 = add i16 %24, 163
  %26 = mul i16 %25, 2
  %27 = getelementptr i8, ptr @"ITEMS%", i16 2
  %28 = load i16, ptr %27, !tbaa !2
  %29 = add i16 804, %26
  %30 = inttoptr i16 %28 to ptr addrspace(2)
  %31 = addrspacecast ptr addrspace(2) %30 to ptr addrspace(1)
  %32 = getelementptr i8, ptr addrspace(1) %31, i16 %29
  %33 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %34 = add i16 789, %33
  store i16 %34, ptr addrspace(1) %32, !tbaa !6
  %35 = load i16, ptr @"STEPCOUNT%", !tbaa !2
  %36 = getelementptr i8, ptr @$data, i16 2
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add nsw i16 %35, %37
  store i16 %38, ptr @"STEPCOUNT%", !tbaa !2
  br label %b2

b6:
  %39 = getelementptr i8, ptr @"ITEMS%", i16 2
  %40 = load i16, ptr %39, !tbaa !2
  %41 = inttoptr i16 %40 to ptr addrspace(2)
  %42 = addrspacecast ptr addrspace(2) %41 to ptr addrspace(1)
  %43 = getelementptr i8, ptr addrspace(1) %42, i16 -2
  %44 = load i16, ptr addrspace(1) %43, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %44)
  %45 = getelementptr i8, ptr @"ITEMS%", i16 2
  %46 = load i16, ptr %45, !tbaa !2
  %47 = inttoptr i16 %46 to ptr addrspace(2)
  %48 = addrspacecast ptr addrspace(2) %47 to ptr addrspace(1)
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 0
  %50 = load i16, ptr addrspace(1) %49, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %50)
  %51 = getelementptr i8, ptr @"ITEMS%", i16 2
  %52 = load i16, ptr %51, !tbaa !2
  %53 = inttoptr i16 %52 to ptr addrspace(2)
  %54 = addrspacecast ptr addrspace(2) %53 to ptr addrspace(1)
  %55 = getelementptr i8, ptr addrspace(1) %54, i16 1934
  %56 = load i16, ptr addrspace(1) %55, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %56)
  %57 = getelementptr i8, ptr @"ITEMS%", i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = inttoptr i16 %58 to ptr addrspace(2)
  %60 = addrspacecast ptr addrspace(2) %59 to ptr addrspace(1)
  %61 = getelementptr i8, ptr addrspace(1) %60, i16 2336
  %62 = load i16, ptr addrspace(1) %61, !tbaa !6
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %62)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string3)
  ret void
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
