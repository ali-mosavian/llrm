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
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i32
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i32 0, ptr %2
  store i16 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  %9 = sub i16 0, 1
  store i16 %9, ptr %8, !tbaa !2
  %10 = sub i16 0, 1
  store i16 %10, ptr %7, !tbaa !2
  %11 = sub i16 0, 156
  store i16 %11, ptr %6, !tbaa !2
  %12 = sub i16 0, 1
  store i16 %12, ptr %5, !tbaa !2
  br label %b2

b2:
  %13 = load i16, ptr %5, !tbaa !2
  %14 = icmp sge i16 %13, 0
  %15 = sext i1 %14 to i16
  %16 = icmp ne i16 %15, 0
  br i1 %16, label %b3, label %b4

b3:
  %17 = load i16, ptr %7, !tbaa !2
  %18 = load i16, ptr %6, !tbaa !2
  %19 = icmp sle i16 %17, %18
  %20 = sext i1 %19 to i16
  %21 = icmp ne i16 %20, 0
  br i1 %21, label %b5, label %b6

b4:
  %22 = load i16, ptr %7, !tbaa !2
  %23 = load i16, ptr %6, !tbaa !2
  %24 = icmp sge i16 %22, %23
  %25 = sext i1 %24 to i16
  %26 = icmp ne i16 %25, 0
  br i1 %26, label %b5, label %b6

b5:
  %27 = load i16, ptr %7, !tbaa !2
  %28 = load i16, ptr %7, !tbaa !2
  %29 = load i16, ptr %7, !tbaa !2
  %30 = sub i16 %29, 63
  %31 = sext i16 %30 to i32
  %32 = sdiv i32 %31, 64
  %33 = trunc i32 %32 to i16
  %34 = mul i16 %33, 64
  %35 = sub i16 %28, %34
  %36 = sub i16 63, %35
  %37 = sub i16 %27, -180
  %38 = getelementptr inbounds i16, ptr @"DT100%", i16 %37
  store i16 %36, ptr %38, !tbaa !2
  %39 = load i16, ptr %8, !tbaa !2
  %40 = icmp eq i16 %39, 1
  %41 = sext i1 %40 to i16
  %42 = icmp ne i16 %41, 0
  br i1 %42, label %b7, label %b8

b6:
  store i16 1, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  store i16 156, ptr %4, !tbaa !2
  store i16 1, ptr %3, !tbaa !2
  br label %b16

b7:
  %43 = load i16, ptr %7, !tbaa !2
  %44 = load i16, ptr %7, !tbaa !2
  %45 = sub i16 %44, -180
  %46 = getelementptr inbounds i16, ptr @"DT100%", i16 %45
  %47 = load i16, ptr %46, !tbaa !2
  %48 = sub i16 63, %47
  %49 = sub i16 %43, -180
  %50 = getelementptr inbounds i16, ptr @"DT100%", i16 %49
  store i16 %48, ptr %50, !tbaa !2
  br label %b9

b8:
  br label %b9

b9:
  %51 = load i16, ptr %7, !tbaa !2
  %52 = sub i16 %51, -180
  %53 = getelementptr inbounds i16, ptr @"DT100%", i16 %52
  %54 = load i16, ptr %53, !tbaa !2
  %55 = icmp eq i16 %54, 0
  %56 = sext i1 %55 to i16
  %57 = load i16, ptr %8, !tbaa !2
  %58 = icmp eq i16 %57, 1
  %59 = sext i1 %58 to i16
  %60 = and i16 %56, %59
  %61 = icmp ne i16 %60, 0
  br i1 %61, label %b10, label %b11

b10:
  %62 = sub i16 0, 1
  store i16 %62, ptr %8, !tbaa !2
  br label %b12

b11:
  br label %b12

b12:
  %63 = load i16, ptr %7, !tbaa !2
  %64 = sub i16 %63, -180
  %65 = getelementptr inbounds i16, ptr @"DT100%", i16 %64
  %66 = load i16, ptr %65, !tbaa !2
  %67 = icmp eq i16 %66, 63
  %68 = sext i1 %67 to i16
  %69 = load i16, ptr %8, !tbaa !2
  %70 = sub i16 0, 1
  %71 = icmp eq i16 %69, %70
  %72 = sext i1 %71 to i16
  %73 = and i16 %68, %72
  %74 = icmp ne i16 %73, 0
  br i1 %74, label %b13, label %b14

b13:
  store i16 1, ptr %8, !tbaa !2
  br label %b15

b14:
  br label %b15

b15:
  %75 = load i16, ptr %7, !tbaa !2
  %76 = load i16, ptr %5, !tbaa !2
  %77 = add i16 %75, %76
  store i16 %77, ptr %7, !tbaa !2
  br label %b2

b16:
  %78 = load i16, ptr %3, !tbaa !2
  %79 = icmp sge i16 %78, 0
  %80 = sext i1 %79 to i16
  %81 = icmp ne i16 %80, 0
  br i1 %81, label %b17, label %b18

b17:
  %82 = load i16, ptr %7, !tbaa !2
  %83 = load i16, ptr %4, !tbaa !2
  %84 = icmp sle i16 %82, %83
  %85 = sext i1 %84 to i16
  %86 = icmp ne i16 %85, 0
  br i1 %86, label %b19, label %b20

b18:
  %87 = load i16, ptr %7, !tbaa !2
  %88 = load i16, ptr %4, !tbaa !2
  %89 = icmp sge i16 %87, %88
  %90 = sext i1 %89 to i16
  %91 = icmp ne i16 %90, 0
  br i1 %91, label %b19, label %b20

b19:
  %92 = load i16, ptr %7, !tbaa !2
  %93 = load i16, ptr %7, !tbaa !2
  %94 = load i16, ptr %7, !tbaa !2
  %95 = sext i16 %94 to i32
  %96 = sdiv i32 %95, 64
  %97 = trunc i32 %96 to i16
  %98 = mul i16 %97, 64
  %99 = sub i16 %93, %98
  %100 = sub i16 %92, -180
  %101 = getelementptr inbounds i16, ptr @"DT100%", i16 %100
  store i16 %99, ptr %101, !tbaa !2
  %102 = load i16, ptr %8, !tbaa !2
  %103 = sub i16 0, 1
  %104 = icmp eq i16 %102, %103
  %105 = sext i1 %104 to i16
  %106 = icmp ne i16 %105, 0
  br i1 %106, label %b21, label %b22

b20:
  store i32 0, ptr %2, !tbaa !2
  %107 = sub i16 0, 156
  store i16 %107, ptr %7, !tbaa !2
  store i16 156, ptr %1, !tbaa !2
  store i16 1, ptr %0, !tbaa !2
  br label %b30

b21:
  %108 = load i16, ptr %7, !tbaa !2
  %109 = load i16, ptr %7, !tbaa !2
  %110 = sub i16 %109, -180
  %111 = getelementptr inbounds i16, ptr @"DT100%", i16 %110
  %112 = load i16, ptr %111, !tbaa !2
  %113 = sub i16 63, %112
  %114 = sub i16 %108, -180
  %115 = getelementptr inbounds i16, ptr @"DT100%", i16 %114
  store i16 %113, ptr %115, !tbaa !2
  br label %b23

b22:
  br label %b23

b23:
  %116 = load i16, ptr %7, !tbaa !2
  %117 = sub i16 %116, -180
  %118 = getelementptr inbounds i16, ptr @"DT100%", i16 %117
  %119 = load i16, ptr %118, !tbaa !2
  %120 = icmp eq i16 %119, 63
  %121 = sext i1 %120 to i16
  %122 = load i16, ptr %8, !tbaa !2
  %123 = icmp eq i16 %122, 1
  %124 = sext i1 %123 to i16
  %125 = and i16 %121, %124
  %126 = icmp ne i16 %125, 0
  br i1 %126, label %b24, label %b25

b24:
  %127 = sub i16 0, 1
  store i16 %127, ptr %8, !tbaa !2
  br label %b26

b25:
  br label %b26

b26:
  %128 = load i16, ptr %7, !tbaa !2
  %129 = sub i16 %128, -180
  %130 = getelementptr inbounds i16, ptr @"DT100%", i16 %129
  %131 = load i16, ptr %130, !tbaa !2
  %132 = icmp eq i16 %131, 0
  %133 = sext i1 %132 to i16
  %134 = load i16, ptr %8, !tbaa !2
  %135 = sub i16 0, 1
  %136 = icmp eq i16 %134, %135
  %137 = sext i1 %136 to i16
  %138 = and i16 %133, %137
  %139 = icmp ne i16 %138, 0
  br i1 %139, label %b27, label %b28

b27:
  store i16 1, ptr %8, !tbaa !2
  br label %b29

b28:
  br label %b29

b29:
  %140 = load i16, ptr %7, !tbaa !2
  %141 = load i16, ptr %3, !tbaa !2
  %142 = add i16 %140, %141
  store i16 %142, ptr %7, !tbaa !2
  br label %b16

b30:
  %143 = load i16, ptr %0, !tbaa !2
  %144 = icmp sge i16 %143, 0
  %145 = sext i1 %144 to i16
  %146 = icmp ne i16 %145, 0
  br i1 %146, label %b31, label %b32

b31:
  %147 = load i16, ptr %7, !tbaa !2
  %148 = load i16, ptr %1, !tbaa !2
  %149 = icmp sle i16 %147, %148
  %150 = sext i1 %149 to i16
  %151 = icmp ne i16 %150, 0
  br i1 %151, label %b33, label %b34

b32:
  %152 = load i16, ptr %7, !tbaa !2
  %153 = load i16, ptr %1, !tbaa !2
  %154 = icmp sge i16 %152, %153
  %155 = sext i1 %154 to i16
  %156 = icmp ne i16 %155, 0
  br i1 %156, label %b33, label %b34

b33:
  %157 = load i32, ptr %2, !tbaa !2
  %158 = load i16, ptr %7, !tbaa !2
  %159 = sub i16 %158, -180
  %160 = getelementptr inbounds i16, ptr @"DT100%", i16 %159
  %161 = load i16, ptr %160, !tbaa !2
  %162 = load i16, ptr %7, !tbaa !2
  %163 = add i16 %162, 200
  %164 = mul i16 %161, %163
  %165 = sext i16 %164 to i32
  %166 = add i32 %157, %165
  store i32 %166, ptr %2, !tbaa !2
  %167 = load i16, ptr %7, !tbaa !2
  %168 = load i16, ptr %0, !tbaa !2
  %169 = add i16 %167, %168
  store i16 %169, ptr %7, !tbaa !2
  br label %b30

b34:
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4$descriptor)
  %170 = load i32, ptr %2, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %170)
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
