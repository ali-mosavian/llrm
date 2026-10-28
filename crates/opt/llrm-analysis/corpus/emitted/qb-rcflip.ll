target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"DT100%" = internal global [722 x i8] zeroinitializer, align 2
@"DT100%$descriptor" = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"DT100%" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr getelementptr (i8, ptr @"DT100%", i16 360), [6 x i8] c"\02\00i\01L\FF" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"SUM=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string5, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  call cc 1000 addrspace(1) void @RAMP()
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @RAMP() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  store i16 -1, ptr %8, !tbaa !2
  store i16 -1, ptr %7, !tbaa !2
  store i16 -156, ptr %6, !tbaa !2
  store i16 -1, ptr %5, !tbaa !2
  br label %b2

b2:
  %9 = load i16, ptr %5, !tbaa !2
  %10 = icmp sge i16 %9, 0
  br i1 %10, label %b3, label %b4

b3:
  %11 = load i16, ptr %7, !tbaa !2
  %12 = load i16, ptr %6, !tbaa !2
  %13 = icmp sle i16 %11, %12
  br i1 %13, label %b5, label %b6

b4:
  %14 = load i16, ptr %7, !tbaa !2
  %15 = load i16, ptr %6, !tbaa !2
  %16 = icmp sge i16 %14, %15
  br i1 %16, label %b5, label %b6

b5:
  %17 = load i16, ptr %7, !tbaa !2
  %18 = load i16, ptr %7, !tbaa !2
  %19 = load i16, ptr %7, !tbaa !2
  %20 = sub i16 %19, 63
  %21 = sext i16 %20 to i32
  %22 = sdiv i32 %21, 64
  %23 = trunc i32 %22 to i16
  %24 = mul i16 %23, 64
  %25 = sub i16 %18, %24
  %26 = sub i16 63, %25
  %27 = sub i16 %17, -180
  %28 = getelementptr inbounds i16, ptr @"DT100%", i16 %27
  store i16 %26, ptr %28, !tbaa !2
  %29 = load i16, ptr %8, !tbaa !2
  %30 = icmp eq i16 %29, 1
  br i1 %30, label %b7, label %b8

b6:
  store i16 1, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 156, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b16

b7:
  %31 = load i16, ptr %7, !tbaa !2
  %32 = load i16, ptr %7, !tbaa !2
  %33 = sub i16 %32, -180
  %34 = getelementptr inbounds i16, ptr @"DT100%", i16 %33
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sub i16 63, %35
  %37 = sub i16 %31, -180
  %38 = getelementptr inbounds i16, ptr @"DT100%", i16 %37
  store i16 %36, ptr %38, !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %39 = load i16, ptr %7, !tbaa !2
  %40 = sub i16 %39, -180
  %41 = getelementptr inbounds i16, ptr @"DT100%", i16 %40
  %42 = load i16, ptr %41, !tbaa !2
  %43 = icmp eq i16 %42, 0
  %44 = sext i1 %43 to i16
  %45 = load i16, ptr %8, !tbaa !2
  %46 = icmp eq i16 %45, 1
  %47 = sext i1 %46 to i16
  %48 = and i16 %44, %47
  %49 = icmp ne i16 %48, 0
  br i1 %49, label %b10, label %b11

b10:
  store i16 -1, ptr %8, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  %50 = load i16, ptr %7, !tbaa !2
  %51 = sub i16 %50, -180
  %52 = getelementptr inbounds i16, ptr @"DT100%", i16 %51
  %53 = load i16, ptr %52, !tbaa !2
  %54 = icmp eq i16 %53, 63
  %55 = sext i1 %54 to i16
  %56 = load i16, ptr %8, !tbaa !2
  %57 = icmp eq i16 %56, -1
  %58 = sext i1 %57 to i16
  %59 = and i16 %55, %58
  %60 = icmp ne i16 %59, 0
  br i1 %60, label %b13, label %b14

b13:
  store i16 1, ptr %8, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  %61 = load i16, ptr %7, !tbaa !2
  %62 = load i16, ptr %5, !tbaa !2
  %63 = add nsw i16 %61, %62
  store i16 %63, ptr %7, !tbaa !2
  br label %b2

b16:
  %64 = load i16, ptr %3, !tbaa !2
  %65 = icmp sge i16 %64, 0
  br i1 %65, label %b17, label %b18

b17:
  %66 = load i16, ptr %7, !tbaa !2
  %67 = load i16, ptr %4, !tbaa !2
  %68 = icmp sle i16 %66, %67
  br i1 %68, label %b19, label %b20

b18:
  %69 = load i16, ptr %7, !tbaa !2
  %70 = load i16, ptr %4, !tbaa !2
  %71 = icmp sge i16 %69, %70
  br i1 %71, label %b19, label %b20

b19:
  %72 = load i16, ptr %7, !tbaa !2
  %73 = load i16, ptr %7, !tbaa !2
  %74 = load i16, ptr %7, !tbaa !2
  %75 = sext i16 %74 to i32
  %76 = sdiv i32 %75, 64
  %77 = trunc i32 %76 to i16
  %78 = mul i16 %77, 64
  %79 = sub i16 %73, %78
  %80 = sub i16 %72, -180
  %81 = getelementptr inbounds i16, ptr @"DT100%", i16 %80
  store i16 %79, ptr %81, !tbaa !2
  %82 = load i16, ptr %8, !tbaa !2
  %83 = icmp eq i16 %82, -1
  br i1 %83, label %b21, label %b22

b20:
  store i32 0, ptr %2, !tbaa !2
  store i16 -156, ptr %7, !tbaa !2
  store i16 156, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b30

b21:
  %84 = load i16, ptr %7, !tbaa !2
  %85 = load i16, ptr %7, !tbaa !2
  %86 = sub i16 %85, -180
  %87 = getelementptr inbounds i16, ptr @"DT100%", i16 %86
  %88 = load i16, ptr %87, !tbaa !2
  %89 = sub i16 63, %88
  %90 = sub i16 %84, -180
  %91 = getelementptr inbounds i16, ptr @"DT100%", i16 %90
  store i16 %89, ptr %91, !tbaa !2
  br label %b23

b22:
  br label %b23

b23:
  %92 = load i16, ptr %7, !tbaa !2
  %93 = sub i16 %92, -180
  %94 = getelementptr inbounds i16, ptr @"DT100%", i16 %93
  %95 = load i16, ptr %94, !tbaa !2
  %96 = icmp eq i16 %95, 63
  %97 = sext i1 %96 to i16
  %98 = load i16, ptr %8, !tbaa !2
  %99 = icmp eq i16 %98, 1
  %100 = sext i1 %99 to i16
  %101 = and i16 %97, %100
  %102 = icmp ne i16 %101, 0
  br i1 %102, label %b24, label %b25

b24:
  store i16 -1, ptr %8, !tbaa !2
  br label %b26

b25:
  br label %b26

b26:
  %103 = load i16, ptr %7, !tbaa !2
  %104 = sub i16 %103, -180
  %105 = getelementptr inbounds i16, ptr @"DT100%", i16 %104
  %106 = load i16, ptr %105, !tbaa !2
  %107 = icmp eq i16 %106, 0
  %108 = sext i1 %107 to i16
  %109 = load i16, ptr %8, !tbaa !2
  %110 = icmp eq i16 %109, -1
  %111 = sext i1 %110 to i16
  %112 = and i16 %108, %111
  %113 = icmp ne i16 %112, 0
  br i1 %113, label %b27, label %b28

b27:
  store i16 1, ptr %8, !tbaa !2
  br label %b29

b28:
  br label %b29

b29:
  %114 = load i16, ptr %7, !tbaa !2
  %115 = load i16, ptr %3, !tbaa !2
  %116 = add nsw i16 %114, %115
  store i16 %116, ptr %7, !tbaa !2
  br label %b16

b30:
  %117 = load i16, ptr %0, !tbaa !2
  %118 = icmp sge i16 %117, 0
  br i1 %118, label %b31, label %b32

b31:
  %119 = load i16, ptr %7, !tbaa !2
  %120 = load i16, ptr %1, !tbaa !2
  %121 = icmp sle i16 %119, %120
  br i1 %121, label %b33, label %b34

b32:
  %122 = load i16, ptr %7, !tbaa !2
  %123 = load i16, ptr %1, !tbaa !2
  %124 = icmp sge i16 %122, %123
  br i1 %124, label %b33, label %b34

b33:
  %125 = load i32, ptr %2, !tbaa !2
  %126 = load i16, ptr %7, !tbaa !2
  %127 = sub i16 %126, -180
  %128 = getelementptr inbounds i16, ptr @"DT100%", i16 %127
  %129 = load i16, ptr %128, !tbaa !2
  %130 = load i16, ptr %7, !tbaa !2
  %131 = add i16 %130, 200
  %132 = mul i16 %129, %131
  %133 = sext i16 %132 to i32
  %134 = add i32 %125, %133
  store i32 %134, ptr %2, !tbaa !2
  %135 = load i16, ptr %7, !tbaa !2
  %136 = load i16, ptr %0, !tbaa !2
  %137 = add nsw i16 %135, %136
  store i16 %137, ptr %7, !tbaa !2
  br label %b30

b34:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %138 = load i32, ptr %2, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %138)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
  ret void
}

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
