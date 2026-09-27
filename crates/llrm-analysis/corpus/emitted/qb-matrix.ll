target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [12 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"M%" = internal global [802 x i8] zeroinitializer
@"M%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"M%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"M%", [6 x i8] c"\02\00\91\01\00\00" }>
@"R%" = internal global [2 x i8] zeroinitializer
@"C%" = internal global [2 x i8] zeroinitializer
@"W%" = internal global [2 x i8] zeroinitializer
@"T%" = internal global [2 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [4 x i8] c"\02\00T=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
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
  %4 = sext i1 %3 to i16
  %5 = icmp ne i16 %4, 0
  br i1 %5, label %b3, label %b4

b3:
  %6 = load i16, ptr @"R%", !tbaa !2
  %7 = load i16, ptr @$data, !tbaa !2
  %8 = icmp sle i16 %6, %7
  %9 = sext i1 %8 to i16
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b5, label %b6

b4:
  %11 = load i16, ptr @"R%", !tbaa !2
  %12 = load i16, ptr @$data, !tbaa !2
  %13 = icmp sge i16 %11, %12
  %14 = sext i1 %13 to i16
  %15 = icmp ne i16 %14, 0
  br i1 %15, label %b5, label %b6

b5:
  store i16 0, ptr @"C%", !tbaa !2
  %16 = getelementptr i8, ptr @$data, i16 4
  store i16 19, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %17, !tbaa !2
  br label %b7

b6:
  store i16 0, ptr @"R%", !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 8
  store i16 19, ptr %18, !tbaa !2
  %19 = getelementptr i8, ptr @$data, i16 10
  store i16 1, ptr %19, !tbaa !2
  br label %b12

b7:
  %20 = getelementptr i8, ptr @$data, i16 6
  %21 = load i16, ptr %20, !tbaa !2
  %22 = icmp sge i16 %21, 0
  %23 = sext i1 %22 to i16
  %24 = icmp ne i16 %23, 0
  br i1 %24, label %b8, label %b9

b8:
  %25 = load i16, ptr @"C%", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 4
  %27 = load i16, ptr %26, !tbaa !2
  %28 = icmp sle i16 %25, %27
  %29 = sext i1 %28 to i16
  %30 = icmp ne i16 %29, 0
  br i1 %30, label %b10, label %b11

b9:
  %31 = load i16, ptr @"C%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 4
  %33 = load i16, ptr %32, !tbaa !2
  %34 = icmp sge i16 %31, %33
  %35 = sext i1 %34 to i16
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %b10, label %b11

b10:
  %37 = load i16, ptr @"R%", !tbaa !2
  %38 = load i16, ptr @"W%", !tbaa !2
  %39 = mul i16 %37, %38
  %40 = load i16, ptr @"C%", !tbaa !2
  %41 = add i16 %39, %40
  %42 = load i16, ptr @"R%", !tbaa !2
  %43 = load i16, ptr @"C%", !tbaa !2
  %44 = add i16 %42, %43
  %45 = sub i16 %41, 0
  %46 = getelementptr inbounds i16, ptr @"M%", i16 %45
  store i16 %44, ptr %46, !tbaa !2
  %47 = load i16, ptr @"C%", !tbaa !2
  %48 = getelementptr i8, ptr @$data, i16 6
  %49 = load i16, ptr %48, !tbaa !2
  %50 = add i16 %47, %49
  store i16 %50, ptr @"C%", !tbaa !2
  br label %b7

b11:
  %51 = load i16, ptr @"R%", !tbaa !2
  %52 = getelementptr i8, ptr @$data, i16 2
  %53 = load i16, ptr %52, !tbaa !2
  %54 = add i16 %51, %53
  store i16 %54, ptr @"R%", !tbaa !2
  br label %b2

b12:
  %55 = getelementptr i8, ptr @$data, i16 10
  %56 = load i16, ptr %55, !tbaa !2
  %57 = icmp sge i16 %56, 0
  %58 = sext i1 %57 to i16
  %59 = icmp ne i16 %58, 0
  br i1 %59, label %b13, label %b14

b13:
  %60 = load i16, ptr @"R%", !tbaa !2
  %61 = getelementptr i8, ptr @$data, i16 8
  %62 = load i16, ptr %61, !tbaa !2
  %63 = icmp sle i16 %60, %62
  %64 = sext i1 %63 to i16
  %65 = icmp ne i16 %64, 0
  br i1 %65, label %b15, label %b16

b14:
  %66 = load i16, ptr @"R%", !tbaa !2
  %67 = getelementptr i8, ptr @$data, i16 8
  %68 = load i16, ptr %67, !tbaa !2
  %69 = icmp sge i16 %66, %68
  %70 = sext i1 %69 to i16
  %71 = icmp ne i16 %70, 0
  br i1 %71, label %b15, label %b16

b15:
  %72 = load i16, ptr @"T%", !tbaa !2
  %73 = load i16, ptr @"R%", !tbaa !2
  %74 = load i16, ptr @"W%", !tbaa !2
  %75 = mul i16 %73, %74
  %76 = load i16, ptr @"R%", !tbaa !2
  %77 = add i16 %75, %76
  %78 = sub i16 %77, 0
  %79 = getelementptr inbounds i16, ptr @"M%", i16 %78
  %80 = load i16, ptr %79, !tbaa !2
  %81 = add i16 %72, %80
  store i16 %81, ptr @"T%", !tbaa !2
  %82 = load i16, ptr @"R%", !tbaa !2
  %83 = getelementptr i8, ptr @$data, i16 10
  %84 = load i16, ptr %83, !tbaa !2
  %85 = add i16 %82, %84
  store i16 %85, ptr @"R%", !tbaa !2
  br label %b12

b16:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %86 = load i16, ptr @"T%", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %86)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
