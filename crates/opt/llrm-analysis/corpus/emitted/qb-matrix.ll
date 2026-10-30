target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"M%" = internal global [802 x i8] zeroinitializer, align 2
@"M%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"M%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"M%", [6 x i8] c"\02\00\91\01\00\00" }>
@"R%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string4, i16 4), [2 x i8] c"T=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 20, ptr @"W%", !tbaa !2
  store i16 0, ptr @"T%", !tbaa !2
  store i16 0, ptr @"R%", !tbaa !2
  store i16 19, ptr @$data, !tbaa !2
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
  store i16 0, ptr @"C%", !tbaa !2
  %10 = getelementptr i8, ptr @$data, i16 4
  store i16 19, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %11, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr @"R%", !tbaa !2
  %12 = getelementptr i8, ptr @$data, i16 8
  store i16 19, ptr %12, !tbaa !2
  %13 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %13, !tbaa !2
  br label %b12

b7:
  %14 = getelementptr i8, ptr @$data, i16 6
  %15 = load i16, ptr %14, !tbaa !2
  %16 = icmp sge i16 %15, 0
  br i1 %16, label %b8, label %b9

b8:
  %17 = load i16, ptr @"C%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 4
  %19 = load i16, ptr %18, !tbaa !2
  %20 = icmp sle i16 %17, %19
  br i1 %20, label %b10, label %b11

b9:
  %21 = load i16, ptr @"C%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 4
  %23 = load i16, ptr %22, !tbaa !2
  %24 = icmp sge i16 %21, %23
  br i1 %24, label %b10, label %b11

b10:
  %25 = load i16, ptr @"R%", !tbaa !2
  %26 = load i16, ptr @"W%", !tbaa !2
  %27 = mul i16 %25, %26
  %28 = load i16, ptr @"C%", !tbaa !2
  %29 = add i16 %27, %28
  %30 = load i16, ptr @"R%", !tbaa !2
  %31 = load i16, ptr @"C%", !tbaa !2
  %32 = add i16 %30, %31
  %33 = sub i16 %29, 0
  %34 = getelementptr inbounds i16, ptr @"M%", i16 %33
  store i16 %32, ptr %34, !tbaa !2
  %35 = load i16, ptr @"C%", !tbaa !2
  %36 = getelementptr i8, ptr @$data, i16 6
  %37 = load i16, ptr %36, !tbaa !2
  %38 = add nsw i16 %35, %37
  store i16 %38, ptr @"C%", !tbaa !2
  br label %b7

b11:
  %39 = load i16, ptr @"R%", !tbaa !2
  %40 = getelementptr i8, ptr @$data, i16 2
  %41 = load i16, ptr %40, !tbaa !2
  %42 = add nsw i16 %39, %41
  store i16 %42, ptr @"R%", !tbaa !2
  br label %b2

b12:
  %43 = getelementptr i8, ptr @$data, i16 10
  %44 = load i16, ptr %43, !tbaa !2
  %45 = icmp sge i16 %44, 0
  br i1 %45, label %b13, label %b14

b13:
  %46 = load i16, ptr @"R%", !tbaa !2
  %47 = getelementptr i8, ptr @$data, i16 8
  %48 = load i16, ptr %47, !tbaa !2
  %49 = icmp sle i16 %46, %48
  br i1 %49, label %b15, label %b16

b14:
  %50 = load i16, ptr @"R%", !tbaa !2
  %51 = getelementptr i8, ptr @$data, i16 8
  %52 = load i16, ptr %51, !tbaa !2
  %53 = icmp sge i16 %50, %52
  br i1 %53, label %b15, label %b16

b15:
  %54 = load i16, ptr @"T%", !tbaa !2
  %55 = load i16, ptr @"R%", !tbaa !2
  %56 = load i16, ptr @"W%", !tbaa !2
  %57 = mul i16 %55, %56
  %58 = load i16, ptr @"R%", !tbaa !2
  %59 = add i16 %57, %58
  %60 = sub i16 %59, 0
  %61 = getelementptr inbounds i16, ptr @"M%", i16 %60
  %62 = load i16, ptr %61, !tbaa !2
  %63 = add i16 %54, %62
  store i16 %63, ptr @"T%", !tbaa !2
  %64 = load i16, ptr @"R%", !tbaa !2
  %65 = getelementptr i8, ptr @$data, i16 10
  %66 = load i16, ptr %65, !tbaa !2
  %67 = add nsw i16 %64, %66
  store i16 %67, ptr @"R%", !tbaa !2
  br label %b12

b16:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %68 = load i16, ptr @"T%", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %68)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
