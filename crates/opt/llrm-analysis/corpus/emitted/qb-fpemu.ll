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
  %0 = load i32, ptr @"X&", !tbaa !2
  %1 = load i32, ptr @"Y&", !tbaa !2
  %2 = sdiv i32 %0, %1
  store i32 %2, ptr @"Q&", !tbaa !2
  %3 = load i32, ptr @"Q&", !tbaa !2
  store i32 %3, ptr @$data, !tbaa !2
  %4 = load i32, ptr @$data, !tbaa !2
  %5 = sitofp i32 %4 to double
  store double %5, ptr @"A#", !tbaa !2
  %6 = load i32, ptr @"Y&", !tbaa !2
  %7 = getelementptr i8, ptr @$data, i16 4
  store i32 %6, ptr %7, !tbaa !2
  %8 = getelementptr i8, ptr @$data, i16 4
  %9 = load i32, ptr %8, !tbaa !2
  %10 = sitofp i32 %9 to double
  store double %10, ptr @"B#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3$descriptor)
  %11 = load i32, ptr @"Q&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %11)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6$descriptor)
  %12 = load double, ptr @"A#", !tbaa !2
  %13 = load double, ptr @"B#", !tbaa !2
  %14 = fadd double %12, %13
  %15 = call i32 @llvm.lrint.i32.f64(double %14)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %15)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8$descriptor)
  %16 = load double, ptr @"A#", !tbaa !2
  %17 = load double, ptr @"B#", !tbaa !2
  %18 = fsub double %16, %17
  %19 = call i32 @llvm.lrint.i32.f64(double %18)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %19)
  %20 = load i32, ptr @"X&", !tbaa !2
  %21 = load i32, ptr @"Y&", !tbaa !2
  %22 = srem i32 %20, %21
  store i32 %22, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10$descriptor)
  %23 = load i32, ptr @"R&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %23)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12$descriptor)
  %24 = load double, ptr @"A#", !tbaa !2
  %25 = load double, ptr @"B#", !tbaa !2
  %26 = fmul double %24, %25
  %27 = call i32 @llvm.lrint.i32.f64(double %26)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %27)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14$descriptor)
  %28 = load double, ptr @"A#", !tbaa !2
  %29 = load double, ptr @"B#", !tbaa !2
  %30 = fdiv double %28, %29
  %31 = call i32 @llvm.lrint.i32.f64(double %30)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %31)
  %32 = load double, ptr @"B#", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 8
  store i16 2048, ptr %33, !tbaa !2
  %34 = getelementptr i8, ptr @$data, i16 8
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sitofp i16 %35 to double
  %37 = fdiv double %32, %36
  store double %37, ptr @"C#", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string16$descriptor)
  %38 = load double, ptr @"C#", !tbaa !2
  %39 = getelementptr i8, ptr @$data, i16 10
  store i16 10, ptr %39, !tbaa !2
  %40 = getelementptr i8, ptr @$data, i16 10
  %41 = load i16, ptr %40, !tbaa !2
  %42 = sitofp i16 %41 to double
  %43 = fmul double %38, %42
  %44 = call i32 @llvm.lrint.i32.f64(double %43)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %44)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string18$descriptor)
  %45 = load double, ptr @"A#", !tbaa !2
  %46 = call double @llvm.sqrt.f64(double %45)
  %47 = call i32 @llvm.lrint.i32.f64(double %46)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %47)
  %48 = load i32, ptr @"Q&", !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 12
  store i32 %48, ptr %49, !tbaa !2
  %50 = getelementptr i8, ptr @$data, i16 12
  %51 = load i32, ptr %50, !tbaa !2
  %52 = sitofp i32 %51 to float
  store float %52, ptr @"S!", !tbaa !2
  %53 = getelementptr i8, ptr @$data, i16 16
  store i16 4, ptr %53, !tbaa !2
  %54 = getelementptr i8, ptr @$data, i16 16
  %55 = load i16, ptr %54, !tbaa !2
  %56 = sitofp i16 %55 to float
  store float %56, ptr @"T!", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string20$descriptor)
  %57 = load float, ptr @"S!", !tbaa !2
  %58 = load float, ptr @"T!", !tbaa !2
  %59 = fadd float %57, %58
  %60 = call i32 @llvm.lrint.i32.f32(float %59)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %60)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string22$descriptor)
  %61 = load float, ptr @"S!", !tbaa !2
  %62 = load float, ptr @"T!", !tbaa !2
  %63 = fmul float %61, %62
  %64 = call i32 @llvm.lrint.i32.f32(float %63)
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %64)
  %65 = load i32, ptr @"X&", !tbaa !2
  %66 = and i32 %65, 1073741824
  store i32 %66, ptr @"M&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string24$descriptor)
  %67 = load i32, ptr @"M&", !tbaa !2
  call cc1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %67)
  call cc1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string26$descriptor)
  %68 = load double, ptr @"A#", !tbaa !2
  %69 = load double, ptr @"B#", !tbaa !2
  %70 = fcmp ogt double %68, %69
  %71 = sext i1 %70 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %71)
  %72 = load double, ptr @"B#", !tbaa !2
  %73 = load double, ptr @"A#", !tbaa !2
  %74 = fcmp ogt double %72, %73
  %75 = sext i1 %74 to i16
  call cc1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %75)
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
