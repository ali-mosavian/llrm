target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$data = internal global [18 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X&" = internal global [4 x i8] zeroinitializer
@"Y&" = internal global [4 x i8] zeroinitializer
@"Q&" = internal global [4 x i8] zeroinitializer
@"R&" = internal global [4 x i8] zeroinitializer
@"M&" = internal global [4 x i8] zeroinitializer
@"A#" = internal global [8 x i8] zeroinitializer
@"B#" = internal global [8 x i8] zeroinitializer
@"C#" = internal global [8 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"T!" = internal global [4 x i8] zeroinitializer
@$fslSegment = internal constant ptr addrspace(2) addrspacecast (ptr addrspace(1) @$string3$payload to ptr addrspace(2))
@$string3$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 4) to i16), [6 x i8] c"\04\00DIV=" }>
@$string3$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string3$payload, i16 2) to i16), ptr @$fslSegment }>
@$string6$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 4) to i16), [8 x i8] c"\05\00FADD=\00" }>
@$string6$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string6$payload, i16 2) to i16), ptr @$fslSegment }>
@$string8$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 4) to i16), [8 x i8] c"\05\00FSUB=\00" }>
@$string8$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string8$payload, i16 2) to i16), ptr @$fslSegment }>
@$string10$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 4) to i16), [6 x i8] c"\04\00MOD=" }>
@$string10$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string10$payload, i16 2) to i16), ptr @$fslSegment }>
@$string12$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 4) to i16), [8 x i8] c"\05\00FMUL=\00" }>
@$string12$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string12$payload, i16 2) to i16), ptr @$fslSegment }>
@$string14$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 4) to i16), [8 x i8] c"\05\00FDIV=\00" }>
@$string14$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string14$payload, i16 2) to i16), ptr @$fslSegment }>
@$string16$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 4) to i16), [8 x i8] c"\06\00FHALF=" }>
@$string16$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string16$payload, i16 2) to i16), ptr @$fslSegment }>
@$string18$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 4) to i16), [8 x i8] c"\05\00FSQR=\00" }>
@$string18$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string18$payload, i16 2) to i16), ptr @$fslSegment }>
@$string20$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 4) to i16), [8 x i8] c"\05\00SADD=\00" }>
@$string20$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string20$payload, i16 2) to i16), ptr @$fslSegment }>
@$string22$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 4) to i16), [8 x i8] c"\05\00SMUL=\00" }>
@$string22$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string22$payload, i16 2) to i16), ptr @$fslSegment }>
@$string24$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 4) to i16), [6 x i8] c"\04\00AND=" }>
@$string24$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string24$payload, i16 2) to i16), ptr @$fslSegment }>
@$string26$payload = internal addrspace(1) constant <{ [2 x i8], i16, [8 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 4) to i16), [8 x i8] c"\05\00FCMP=\00" }>
@$string26$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string26$payload, i16 2) to i16), ptr @$fslSegment }>
@$string28$payload = internal addrspace(1) constant <{ [2 x i8], i16, [6 x i8] }> <{ [2 x i8] zeroinitializer, i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 4) to i16), [6 x i8] c"\04\00DONE" }>
@$string28$descriptor = internal constant <{ i16, ptr }> <{ i16 ptrtoint (ptr addrspace(1) getelementptr (i8, ptr addrspace(1) @$string28$payload, i16 2) to i16), ptr @$fslSegment }>

define internal cc1000 void @__main() addrspace(1) {
b1:
  store i32 1073741831, ptr @"X&", !tbaa !2
  store i32 1024, ptr @"Y&", !tbaa !2
  store i32 1048576, ptr @"Q&", !tbaa !2
  store i32 1048576, ptr @$data, !tbaa !2
  store double 1.048576e+06, ptr @"A#", !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 4
  store i32 1024, ptr %0, !tbaa !2
  store double 1.024000e+03, ptr @"B#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %1 = load i32, ptr @"Q&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %1)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %2 = load double, ptr @"A#", !tbaa !2
  %3 = load double, ptr @"B#", !tbaa !2
  %4 = fadd double %2, %3
  %5 = call i32 @llvm.lrint.i32.f64(double %4)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %5)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %6 = load double, ptr @"A#", !tbaa !2
  %7 = load double, ptr @"B#", !tbaa !2
  %8 = fsub double %6, %7
  %9 = call i32 @llvm.lrint.i32.f64(double %8)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %9)
  %10 = load i32, ptr @"X&", !tbaa !2
  %11 = load i32, ptr @"Y&", !tbaa !2
  %12 = srem i32 %10, %11
  store i32 %12, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %13 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %13)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %14 = load double, ptr @"A#", !tbaa !2
  %15 = load double, ptr @"B#", !tbaa !2
  %16 = fmul double %14, %15
  %17 = call i32 @llvm.lrint.i32.f64(double %16)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %17)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %18 = load double, ptr @"A#", !tbaa !2
  %19 = load double, ptr @"B#", !tbaa !2
  %20 = fdiv double %18, %19
  %21 = call i32 @llvm.lrint.i32.f64(double %20)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %21)
  %22 = load double, ptr @"B#", !tbaa !2
  %23 = getelementptr i8, ptr @$data, i16 8
  store i16 2048, ptr %23, !tbaa !2
  %24 = fdiv double %22, 2.048000e+03
  store double %24, ptr @"C#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %25 = load double, ptr @"C#", !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 10
  store i16 10, ptr %26, !tbaa !2
  %27 = fmul double %25, 1.000000e+01
  %28 = call i32 @llvm.lrint.i32.f64(double %27)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %28)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18$descriptor)
  %29 = load double, ptr @"A#", !tbaa !2
  %30 = call double @llvm.sqrt.f64(double %29)
  %31 = call i32 @llvm.lrint.i32.f64(double %30)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %31)
  %32 = load i32, ptr @"Q&", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 12
  store i32 %32, ptr %33, !tbaa !2
  %34 = sitofp i32 %32 to float
  store float %34, ptr @"S!", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 16
  store i16 4, ptr %35, !tbaa !2
  store float 4.000000e+00, ptr @"T!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20$descriptor)
  %36 = load float, ptr @"S!", !tbaa !2
  %37 = load float, ptr @"T!", !tbaa !2
  %38 = fadd float %36, %37
  %39 = call i32 @llvm.lrint.i32.f32(float %38)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %39)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22$descriptor)
  %40 = load float, ptr @"S!", !tbaa !2
  %41 = load float, ptr @"T!", !tbaa !2
  %42 = fmul float %40, %41
  %43 = call i32 @llvm.lrint.i32.f32(float %42)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %43)
  %44 = load i32, ptr @"X&", !tbaa !2
  %45 = and i32 %44, 1073741824
  store i32 %45, ptr @"M&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string24$descriptor)
  %46 = load i32, ptr @"M&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %46)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string26$descriptor)
  %47 = load double, ptr @"A#", !tbaa !2
  %48 = load double, ptr @"B#", !tbaa !2
  %49 = fcmp ogt double %47, %48
  %50 = sext i1 %49 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %50)
  %51 = load double, ptr @"B#", !tbaa !2
  %52 = load double, ptr @"A#", !tbaa !2
  %53 = fcmp ogt double %51, %52
  %54 = sext i1 %53 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %54)
  call cc1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string28$descriptor)
  call cc1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sqrt.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
