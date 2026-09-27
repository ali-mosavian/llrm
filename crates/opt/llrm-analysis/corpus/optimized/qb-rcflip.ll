target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"DT100%" = internal global [722 x i8] zeroinitializer
@"DT100%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DT100%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"DT100%", i16 360), [6 x i8] c"\02\00i\01L\FF" }>
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string4$payload to ptr addrspace(2))
@$string4$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 4) to i16), [6 x i8] c"\04\00SUM=" }>
@$string4$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string4$payload, i16 2) to i16), ptr @$fslSegment }>
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  call cc1000 addrspace(1) void @RAMP()
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc1000 void @RAMP() addrspace(1) {
b1:
  br label %b2

b2:
  %0 = phi i16 [ -1, %b1 ], [ %29, %b15 ]
  %1 = phi i16 [ -1, %b1 ], [ %30, %b15 ]
  %2 = icmp sge i16 %1, -156
  br i1 %2, label %b5, label %b6

b5:
  %3 = add i16 %1, -63
  %4 = sext i16 %3 to i32
  %5 = sdiv i32 %4, 64
  %6 = trunc i32 %5 to i16
  %7 = shl i16 %6, 6
  %8 = sub i16 %1, %7
  %9 = sub i16 63, %8
  %10 = add i16 %1, 180
  %11 = getelementptr inbounds i16, ptr @"DT100%", i16 %10
  store i16 %9, ptr %11, !tbaa !2
  %12 = icmp eq i16 %0, 1
  br i1 %12, label %b7, label %b9

b6:
  br label %b16

b7:
  %13 = load i16, ptr %11, !tbaa !2
  %14 = sub i16 63, %13
  store i16 %14, ptr %11, !tbaa !2
  br label %b9

b9:
  %15 = load i16, ptr %11, !tbaa !2
  %16 = icmp eq i16 %15, 0
  %17 = sext i1 %16 to i16
  %18 = sext i1 %12 to i16
  %19 = and i16 %17, %18
  %20 = icmp ne i16 %19, 0
  br i1 %20, label %b12, label %b11

b11:
  br label %b12

b12:
  %21 = phi i16 [ -1, %b9 ], [ %0, %b11 ]
  %22 = load i16, ptr %11, !tbaa !2
  %23 = icmp eq i16 %22, 63
  %24 = sext i1 %23 to i16
  %25 = icmp eq i16 %21, -1
  %26 = sext i1 %25 to i16
  %27 = and i16 %24, %26
  %28 = icmp ne i16 %27, 0
  br i1 %28, label %b15, label %b14

b14:
  br label %b15

b15:
  %29 = phi i16 [ 1, %b12 ], [ %21, %b14 ]
  %30 = add i16 %1, -1
  br label %b2

b16:
  %31 = phi i16 [ 1, %b6 ], [ %59, %b29 ]
  %32 = phi i16 [ 0, %b6 ], [ %60, %b29 ]
  %33 = icmp sle i16 %32, 156
  br i1 %33, label %b19, label %b20

b19:
  %34 = sext i16 %32 to i32
  %35 = sdiv i32 %34, 64
  %36 = trunc i32 %35 to i16
  %37 = shl i16 %36, 6
  %38 = sub i16 %32, %37
  %39 = add i16 %32, 180
  %40 = getelementptr inbounds i16, ptr @"DT100%", i16 %39
  store i16 %38, ptr %40, !tbaa !2
  %41 = icmp eq i16 %31, -1
  br i1 %41, label %b21, label %b23

b20:
  br label %b30

b21:
  %42 = load i16, ptr %40, !tbaa !2
  %43 = sub i16 63, %42
  store i16 %43, ptr %40, !tbaa !2
  br label %b23

b23:
  %44 = load i16, ptr %40, !tbaa !2
  %45 = icmp eq i16 %44, 63
  %46 = sext i1 %45 to i16
  %47 = icmp eq i16 %31, 1
  %48 = sext i1 %47 to i16
  %49 = and i16 %46, %48
  %50 = icmp ne i16 %49, 0
  br i1 %50, label %b26, label %b25

b25:
  br label %b26

b26:
  %51 = phi i16 [ -1, %b23 ], [ %31, %b25 ]
  %52 = load i16, ptr %40, !tbaa !2
  %53 = icmp eq i16 %52, 0
  %54 = sext i1 %53 to i16
  %55 = icmp eq i16 %51, -1
  %56 = sext i1 %55 to i16
  %57 = and i16 %54, %56
  %58 = icmp ne i16 %57, 0
  br i1 %58, label %b29, label %b28

b28:
  br label %b29

b29:
  %59 = phi i16 [ 1, %b26 ], [ %51, %b28 ]
  %60 = add i16 %32, 1
  br label %b16

b30:
  %61 = phi i16 [ -156, %b20 ], [ %71, %b33 ]
  %62 = phi i32 [ 0, %b20 ], [ %70, %b33 ]
  %63 = icmp sle i16 %61, 156
  br i1 %63, label %b33, label %b34

b33:
  %64 = add i16 %61, 180
  %65 = getelementptr inbounds i16, ptr @"DT100%", i16 %64
  %66 = load i16, ptr %65, !tbaa !2
  %67 = add i16 %61, 200
  %68 = mul i16 %66, %67
  %69 = sext i16 %68 to i32
  %70 = add i32 %62, %69
  %71 = add i16 %61, 1
  br label %b30

b34:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %62)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7$descriptor)
  ret void
}

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
