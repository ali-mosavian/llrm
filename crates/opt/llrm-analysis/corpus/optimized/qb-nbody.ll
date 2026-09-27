target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [28 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"POSX&" = internal global [28 x i8] zeroinitializer
@POSX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"POSY&" = internal global [28 x i8] zeroinitializer
@POSY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELX&" = internal global [28 x i8] zeroinitializer
@VELX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELX&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELX&", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELY&" = internal global [28 x i8] zeroinitializer
@VELY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELY&" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELY&", [6 x i8] c"\04\00\07\00\00\00" }>
@"DELTAX&" = internal global [4 x i8] zeroinitializer
@"DELTAY&" = internal global [4 x i8] zeroinitializer
@"DIST2&" = internal global [4 x i8] zeroinitializer
@"FALLOFF&" = internal global [4 x i8] zeroinitializer
@"ACCX&" = internal global [4 x i8] zeroinitializer
@"ACCY&" = internal global [4 x i8] zeroinitializer
@"STEPCOUNT&" = internal global [4 x i8] zeroinitializer
@"STEPNO&" = internal global [4 x i8] zeroinitializer
@"BODY%" = internal global [2 x i8] zeroinitializer
@"OTHER%" = internal global [2 x i8] zeroinitializer
@TAG$ = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string7$payload to ptr addrspace(2))
@$string7$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 4) to i16), [4 x i8] c"\02\00PX" }>
@$string7$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string7$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [4 x i8] c"\02\00PY" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [4 x i8] c"\02\00VX" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [4 x i8] c"\02\00VY" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [4 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [4 x i8] c"\01\00=\00" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  %0 = call cc1000 addrspace(1) ptr @llrm.qb.B$FCMD()
  %1 = call cc1000 addrspace(1) ptr @llrm.qb.B$FVAL(ptr %0)
  %2 = load double, ptr %1
  %3 = call i32 @llvm.lrint.i32.f64(double %2)
  store i32 %3, ptr @"STEPCOUNT&", !tbaa !2
  %4 = icmp sle i32 %3, 0
  br i1 %4, label %b2, label %b4

b2:
  store i32 100, ptr @"STEPCOUNT&", !tbaa !2
  br label %b4

b4:
  store i16 0, ptr @"BODY%", !tbaa !2
  store i16 5, ptr @$data, !tbaa !2
  %5 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %5, !tbaa !2
  br label %b5

b5:
  %6 = load i16, ptr @"BODY%", !tbaa !2
  %7 = icmp sle i16 %6, 5
  br i1 %7, label %b8, label %b9

b8:
  %8 = mul i16 %6, 7
  %9 = add i16 %8, -15
  %10 = sext i16 %9 to i32
  %11 = shl i32 %10, 9
  %12 = getelementptr inbounds i32, ptr @"POSX&", i16 %6
  store i32 %11, ptr %12, !tbaa !2
  %13 = mul i16 %6, 5
  %14 = add i16 %13, -12
  %15 = sext i16 %14 to i32
  %16 = shl i32 %15, 9
  %17 = getelementptr inbounds i32, ptr @"POSY&", i16 %6
  store i32 %16, ptr %17, !tbaa !2
  %18 = getelementptr inbounds i32, ptr @"VELX&", i16 %6
  store i32 0, ptr %18, !tbaa !2
  %19 = getelementptr inbounds i32, ptr @"VELY&", i16 %6
  store i32 0, ptr %19, !tbaa !2
  %20 = add i16 %6, 1
  store i16 %20, ptr @"BODY%", !tbaa !2
  br label %b5

b9:
  store i32 1, ptr @"STEPNO&", !tbaa !2
  %21 = load i32, ptr @"STEPCOUNT&", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 4
  store i32 %21, ptr %22, !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 8
  store i32 1, ptr %23, !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 12
  %25 = getelementptr i8, ptr @$data, i16 14
  %26 = getelementptr i8, ptr @$data, i16 16
  %27 = getelementptr i8, ptr @$data, i16 18
  %28 = getelementptr i8, ptr @$data, i16 20
  %29 = getelementptr i8, ptr @$data, i16 22
  br label %b11

b11:
  %30 = load i32, ptr @"STEPNO&", !tbaa !2
  %31 = icmp sle i32 %30, %21
  br i1 %31, label %b13, label %b14

b13:
  store i16 0, ptr @"BODY%", !tbaa !2
  store i16 5, ptr %24, !tbaa !2
  store i16 1, ptr %25, !tbaa !2
  br label %b15

b14:
  store i16 0, ptr @"BODY%", !tbaa !2
  %32 = getelementptr i8, ptr @$data, i16 24
  store i16 5, ptr %32, !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 26
  store i16 1, ptr %33, !tbaa !2
  br label %b33

b15:
  %34 = load i16, ptr @"BODY%", !tbaa !2
  %35 = icmp sle i16 %34, 5
  br i1 %35, label %b18, label %b19

b18:
  store i32 0, ptr @"ACCX&", !tbaa !2
  store i32 0, ptr @"ACCY&", !tbaa !2
  store i16 0, ptr @"OTHER%", !tbaa !2
  store i16 5, ptr %26, !tbaa !2
  store i16 1, ptr %27, !tbaa !2
  %36 = getelementptr inbounds i32, ptr @"POSX&", i16 %34
  %37 = getelementptr inbounds i32, ptr @"POSY&", i16 %34
  br label %b21

b19:
  store i16 0, ptr @"BODY%", !tbaa !2
  store i16 5, ptr %28, !tbaa !2
  store i16 1, ptr %29, !tbaa !2
  br label %b28

b21:
  %38 = load i16, ptr @"OTHER%", !tbaa !2
  %39 = icmp sle i16 %38, 5
  br i1 %39, label %b23, label %b24

b23:
  %40 = icmp ne i16 %38, %34
  br i1 %40, label %b25, label %b27

b24:
  %41 = load i16, ptr @"BODY%", !tbaa !2
  %42 = getelementptr inbounds i32, ptr @"VELX&", i16 %41
  %43 = load i32, ptr %42, !tbaa !2
  %44 = load i32, ptr @"ACCX&", !tbaa !2
  %45 = add i32 %43, %44
  store i32 %45, ptr %42, !tbaa !2
  %46 = getelementptr inbounds i32, ptr @"VELY&", i16 %41
  %47 = load i32, ptr %46, !tbaa !2
  %48 = load i32, ptr @"ACCY&", !tbaa !2
  %49 = add i32 %47, %48
  store i32 %49, ptr %46, !tbaa !2
  %50 = load i32, ptr %42, !tbaa !2
  %51 = load i32, ptr %42, !tbaa !2
  %52 = sdiv i32 %51, 16
  %53 = sub i32 %50, %52
  store i32 %53, ptr %42, !tbaa !2
  %54 = load i32, ptr %46, !tbaa !2
  %55 = load i32, ptr %46, !tbaa !2
  %56 = sdiv i32 %55, 16
  %57 = sub i32 %54, %56
  store i32 %57, ptr %46, !tbaa !2
  %58 = add i16 %41, 1
  store i16 %58, ptr @"BODY%", !tbaa !2
  br label %b15

b25:
  %59 = getelementptr inbounds i32, ptr @"POSX&", i16 %38
  %60 = load i32, ptr %59, !tbaa !2
  %61 = load i32, ptr %36, !tbaa !2
  %62 = sub i32 %60, %61
  store i32 %62, ptr @"DELTAX&", !tbaa !2
  %63 = getelementptr inbounds i32, ptr @"POSY&", i16 %38
  %64 = load i32, ptr %63, !tbaa !2
  %65 = load i32, ptr %37, !tbaa !2
  %66 = sub i32 %64, %65
  store i32 %66, ptr @"DELTAY&", !tbaa !2
  %67 = mul i32 %62, %62
  %68 = mul i32 %66, %66
  %69 = add i32 %67, %68
  %70 = add i32 %69, 262144
  store i32 %70, ptr @"DIST2&", !tbaa !2
  %71 = sdiv i32 %70, 262144
  %72 = add i32 %71, 1
  %73 = sdiv i32 512, %72
  store i32 %73, ptr @"FALLOFF&", !tbaa !2
  %74 = load i32, ptr @"ACCX&", !tbaa !2
  %75 = mul i32 %62, %73
  %76 = sdiv i32 %75, 512
  %77 = add i32 %74, %76
  store i32 %77, ptr @"ACCX&", !tbaa !2
  %78 = load i32, ptr @"ACCY&", !tbaa !2
  %79 = mul i32 %66, %73
  %80 = sdiv i32 %79, 512
  %81 = add i32 %78, %80
  store i32 %81, ptr @"ACCY&", !tbaa !2
  br label %b27

b27:
  %82 = load i16, ptr @"OTHER%", !tbaa !2
  %83 = add i16 %82, 1
  store i16 %83, ptr @"OTHER%", !tbaa !2
  br label %b21

b28:
  %84 = load i16, ptr @"BODY%", !tbaa !2
  %85 = icmp sle i16 %84, 5
  br i1 %85, label %b31, label %b32

b31:
  %86 = getelementptr inbounds i32, ptr @"POSX&", i16 %84
  %87 = load i32, ptr %86, !tbaa !2
  %88 = getelementptr inbounds i32, ptr @"VELX&", i16 %84
  %89 = load i32, ptr %88, !tbaa !2
  %90 = add i32 %87, %89
  store i32 %90, ptr %86, !tbaa !2
  %91 = getelementptr inbounds i32, ptr @"POSY&", i16 %84
  %92 = load i32, ptr %91, !tbaa !2
  %93 = getelementptr inbounds i32, ptr @"VELY&", i16 %84
  %94 = load i32, ptr %93, !tbaa !2
  %95 = add i32 %92, %94
  store i32 %95, ptr %91, !tbaa !2
  %96 = add i16 %84, 1
  store i16 %96, ptr @"BODY%", !tbaa !2
  br label %b28

b32:
  %97 = load i32, ptr @"STEPNO&", !tbaa !2
  %98 = add i32 %97, 1
  store i32 %98, ptr @"STEPNO&", !tbaa !2
  br label %b11

b33:
  %99 = load i16, ptr %33, !tbaa !2
  %100 = icmp sge i16 %99, 0
  br i1 %100, label %b34, label %b35

b34:
  %101 = load i16, ptr @"BODY%", !tbaa !2
  %102 = load i16, ptr %32, !tbaa !2
  %103 = icmp sle i16 %101, %102
  br i1 %103, label %b36, label %b37

b35:
  %104 = load i16, ptr @"BODY%", !tbaa !2
  %105 = load i16, ptr %32, !tbaa !2
  %106 = icmp sge i16 %104, %105
  br i1 %106, label %b36, label %b37

b36:
  %107 = load i16, ptr @"BODY%", !tbaa !2
  %108 = call cc1000 addrspace(1) ptr @llrm.qb.B$STI2(i16 %107)
  %109 = call cc1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %108)
  call cc1000 addrspace(1) void @llrm.qb.B$SASS(ptr %109, ptr @TAG$)
  %110 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string7$descriptor, ptr @TAG$)
  %111 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %110, ptr @$string10$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %111)
  %112 = load i16, ptr @"BODY%", !tbaa !2
  %113 = getelementptr inbounds i32, ptr @"POSX&", i16 %112
  %114 = load i32, ptr %113, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %114)
  %115 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string12$descriptor, ptr @TAG$)
  %116 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %115, ptr @$string14$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %116)
  %117 = load i16, ptr @"BODY%", !tbaa !2
  %118 = getelementptr inbounds i32, ptr @"POSY&", i16 %117
  %119 = load i32, ptr %118, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %119)
  %120 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string16$descriptor, ptr @TAG$)
  %121 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %120, ptr @$string18$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %121)
  %122 = load i16, ptr @"BODY%", !tbaa !2
  %123 = getelementptr inbounds i32, ptr @"VELX&", i16 %122
  %124 = load i32, ptr %123, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %124)
  %125 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr @$string20$descriptor, ptr @TAG$)
  %126 = call cc1000 addrspace(1) ptr @llrm.qb.B$SCAT(ptr %125, ptr @$string22$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr %126)
  %127 = load i16, ptr @"BODY%", !tbaa !2
  %128 = getelementptr inbounds i32, ptr @"VELY&", i16 %127
  %129 = load i32, ptr %128, !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %129)
  %130 = load i16, ptr @"BODY%", !tbaa !2
  %131 = load i16, ptr %33, !tbaa !2
  %132 = add i16 %130, %131
  store i16 %132, ptr @"BODY%", !tbaa !2
  br label %b33

b37:
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string24$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 ptr @llrm.qb.B$FCMD() addrspace(1)

declare cc1000 ptr @llrm.qb.B$FVAL(ptr) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 ptr @llrm.qb.B$STI2(i16) addrspace(1)

declare cc1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$SASS(ptr, ptr) addrspace(1)

declare cc1000 ptr @llrm.qb.B$SCAT(ptr, ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
