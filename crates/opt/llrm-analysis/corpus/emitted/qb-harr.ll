target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"M%" = internal global [22 x i8] zeroinitializer
@"R%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string3 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string3, i16 4), [2 x i8] c"T=" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @llrm.qb.B$DDIM(i16 0, i16 20, i16 0, i16 20, i16 2, i16 258, ptr @"M%")
  store i16 0, ptr @"T%", !tbaa !2
  store i16 1, ptr @"R%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b2

b2:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b3, label %b4

b3:
  %4 = load i16, ptr @"R%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b5, label %b6

b4:
  %7 = load i16, ptr @"R%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b5, label %b6

b5:
  store i16 1, ptr @"C%", !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 4
  store i16 10, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %11, !tbaa !2
  br label %b7

b6:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
  %12 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %12)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string4)
  ret void

b7:
  %13 = getelementptr i8, ptr @$data, i16 6
  %14 = load i16, ptr %13, !tbaa !2
  %15 = icmp sge i16 %14, 0
  br i1 %15, label %b8, label %b9

b8:
  %16 = load i16, ptr @"C%", !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 4
  %18 = load i16, ptr %17, !tbaa !2
  %19 = icmp sle i16 %16, %18
  br i1 %19, label %b10, label %b11

b9:
  %20 = load i16, ptr @"C%", !tbaa !2
  %21 = getelementptr i8, ptr @$data, i16 4
  %22 = load i16, ptr %21, !tbaa !2
  %23 = icmp sge i16 %20, %22
  br i1 %23, label %b10, label %b11

b10:
  %24 = load i16, ptr @"R%", !tbaa !2
  %25 = load i16, ptr @"C%", !tbaa !2
  %26 = mul i16 %25, 21
  %27 = add i16 %26, %24
  %28 = mul i16 %27, 2
  %29 = getelementptr i8, ptr @"M%", i16 2
  %30 = load i16, ptr %29, !tbaa !2
  %31 = add i16 0, %28
  %32 = inttoptr i16 %30 to ptr addrspace(2)
  %33 = addrspacecast ptr addrspace(2) %32 to ptr addrspace(1)
  %34 = getelementptr i8, ptr addrspace(1) %33, i16 %31
  %35 = load i16, ptr @"R%", !tbaa !2
  %36 = load i16, ptr @"C%", !tbaa !2
  %37 = add i16 %35, %36
  store i16 %37, ptr addrspace(1) %34, !tbaa !6
  %38 = load i16, ptr @"T%", !tbaa !2
  %39 = load i16, ptr @"R%", !tbaa !2
  %40 = load i16, ptr @"C%", !tbaa !2
  %41 = mul i16 %40, 21
  %42 = add i16 %41, %39
  %43 = mul i16 %42, 2
  %44 = getelementptr i8, ptr @"M%", i16 2
  %45 = load i16, ptr %44, !tbaa !2
  %46 = add i16 0, %43
  %47 = inttoptr i16 %45 to ptr addrspace(2)
  %48 = addrspacecast ptr addrspace(2) %47 to ptr addrspace(1)
  %49 = getelementptr i8, ptr addrspace(1) %48, i16 %46
  %50 = load i16, ptr addrspace(1) %49, !tbaa !6
  %51 = add i16 %38, %50
  store i16 %51, ptr @"T%", !tbaa !2
  %52 = load i16, ptr @"C%", !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 6
  %54 = load i16, ptr %53, !tbaa !2
  %55 = add nsw i16 %52, %54
  store i16 %55, ptr @"C%", !tbaa !2
  br label %b7

b11:
  %56 = load i16, ptr @"R%", !tbaa !2
  %57 = getelementptr i8, ptr @$data, i16 2
  %58 = load i16, ptr %57, !tbaa !2
  %59 = add nsw i16 %56, %58
  store i16 %59, ptr @"R%", !tbaa !2
  br label %b2
}

declare cc 1000 void @llrm.qb.B$DDIM(i16, i16, i16, i16, i16, i16, ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"allocation.0", !3, i64 0}
!6 = !{!5, !5, i64 0}
