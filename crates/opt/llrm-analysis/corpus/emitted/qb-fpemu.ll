target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
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
@$arguments$12 = internal global [18 x i8] zeroinitializer
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"DIV=" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"FADD=\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"FSUB=\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string7, i16 4), [4 x i8] c"MOD=" }>
@$string8 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string8, i16 4), [6 x i8] c"FMUL=\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"FDIV=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"FHALF=" }>
@$string11 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string11, i16 4), [6 x i8] c"FSQR=\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string12, i16 4), [6 x i8] c"SADD=\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string13, i16 4), [6 x i8] c"SMUL=\00" }>
@$string14 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string14, i16 4), [4 x i8] c"AND=" }>
@$string15 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string15, i16 4), [6 x i8] c"FCMP=\00" }>
@$string16 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string16, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i32 1073741831, ptr @"X&", !tbaa !2
  store i32 1024, ptr @"Y&", !tbaa !2
  %0 = load i32, ptr @"X&", !tbaa !2
  %1 = load i32, ptr @"Y&", !tbaa !2
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %b2, label %b3

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b3:
  %3 = icmp eq i32 %1, -1
  br i1 %3, label %b4, label %b5

b4:
  %4 = sub i32 0, %0
  store i32 %4, ptr @$data, !tbaa !2
  br label %b6

b5:
  %5 = sdiv i32 %0, %1
  store i32 %5, ptr @$data, !tbaa !2
  br label %b6

b6:
  %6 = load i32, ptr @$data, !tbaa !2
  store i32 %6, ptr @"Q&", !tbaa !2
  %7 = load i32, ptr @"Q&", !tbaa !2
  store i32 %7, ptr @$arguments$12, !tbaa !2
  %8 = load i32, ptr @$arguments$12, !tbaa !2
  %9 = sitofp i32 %8 to double
  %10 = fpround double %9 to double
  store double %10, ptr @"A#", !tbaa !2
  %11 = load i32, ptr @"Y&", !tbaa !2
  %12 = getelementptr i8, ptr @$arguments$12, i16 4
  store i32 %11, ptr %12, !tbaa !2
  %13 = getelementptr i8, ptr @$arguments$12, i16 4
  %14 = load i32, ptr %13, !tbaa !2
  %15 = sitofp i32 %14 to double
  %16 = fpround double %15 to double
  store double %16, ptr @"B#", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %17 = load i32, ptr @"Q&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %17)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  %18 = load double, ptr @"A#", !tbaa !2
  %19 = load double, ptr @"B#", !tbaa !2
  %20 = fadd double %18, %19
  %21 = call i32 @llvm.lrint.i32.f64(double %20)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %21)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %22 = load double, ptr @"A#", !tbaa !2
  %23 = load double, ptr @"B#", !tbaa !2
  %24 = fsub double %22, %23
  %25 = call i32 @llvm.lrint.i32.f64(double %24)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %25)
  %26 = load i32, ptr @"X&", !tbaa !2
  %27 = load i32, ptr @"Y&", !tbaa !2
  %28 = icmp eq i32 %27, 0
  br i1 %28, label %b7, label %b8

b7:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11) cold
  unreachable

b8:
  %29 = icmp eq i32 %27, -1
  br i1 %29, label %b9, label %b10

b9:
  %30 = getelementptr i8, ptr @$data, i16 4
  store i32 0, ptr %30, !tbaa !2
  br label %b11

b10:
  %31 = srem i32 %26, %27
  %32 = getelementptr i8, ptr @$data, i16 4
  store i32 %31, ptr %32, !tbaa !2
  br label %b11

b11:
  %33 = getelementptr i8, ptr @$data, i16 4
  %34 = load i32, ptr %33, !tbaa !2
  store i32 %34, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
  %35 = load i32, ptr @"R&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %35)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %36 = load double, ptr @"A#", !tbaa !2
  %37 = load double, ptr @"B#", !tbaa !2
  %38 = fmul double %36, %37
  %39 = call i32 @llvm.lrint.i32.f64(double %38)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %39)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  %40 = load double, ptr @"A#", !tbaa !2
  %41 = load double, ptr @"B#", !tbaa !2
  %42 = fdiv double %40, %41
  %43 = call i32 @llvm.lrint.i32.f64(double %42)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %43)
  %44 = load double, ptr @"B#", !tbaa !2
  %45 = getelementptr i8, ptr @$arguments$12, i16 8
  store i16 2048, ptr %45, !tbaa !2
  %46 = getelementptr i8, ptr @$arguments$12, i16 8
  %47 = load i16, ptr %46, !tbaa !2
  %48 = sitofp i16 %47 to double
  %49 = fdiv double %44, %48
  %50 = fpround double %49 to double
  store double %50, ptr @"C#", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  %51 = load double, ptr @"C#", !tbaa !2
  %52 = getelementptr i8, ptr @$arguments$12, i16 10
  store i16 10, ptr %52, !tbaa !2
  %53 = getelementptr i8, ptr @$arguments$12, i16 10
  %54 = load i16, ptr %53, !tbaa !2
  %55 = sitofp i16 %54 to double
  %56 = fmul double %51, %55
  %57 = call i32 @llvm.lrint.i32.f64(double %56)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %57)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string11)
  %58 = load double, ptr @"A#", !tbaa !2
  %59 = call double @llvm.sqrt.f64(double %58)
  %60 = call i32 @llvm.lrint.i32.f64(double %59)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %60)
  %61 = load i32, ptr @"Q&", !tbaa !2
  %62 = getelementptr i8, ptr @$arguments$12, i16 12
  store i32 %61, ptr %62, !tbaa !2
  %63 = getelementptr i8, ptr @$arguments$12, i16 12
  %64 = load i32, ptr %63, !tbaa !2
  %65 = sitofp i32 %64 to float
  %66 = fpround float %65 to float
  store float %66, ptr @"S!", !tbaa !2
  %67 = getelementptr i8, ptr @$arguments$12, i16 16
  store i16 4, ptr %67, !tbaa !2
  %68 = getelementptr i8, ptr @$arguments$12, i16 16
  %69 = load i16, ptr %68, !tbaa !2
  %70 = sitofp i16 %69 to float
  %71 = fpround float %70 to float
  store float %71, ptr @"T!", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string12)
  %72 = load float, ptr @"S!", !tbaa !2
  %73 = load float, ptr @"T!", !tbaa !2
  %74 = fadd float %72, %73
  %75 = call i32 @llvm.lrint.i32.f32(float %74)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %75)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  %76 = load float, ptr @"S!", !tbaa !2
  %77 = load float, ptr @"T!", !tbaa !2
  %78 = fmul float %76, %77
  %79 = call i32 @llvm.lrint.i32.f32(float %78)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %79)
  %80 = load i32, ptr @"X&", !tbaa !2
  %81 = and i32 %80, 1073741824
  store i32 %81, ptr @"M&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string14)
  %82 = load i32, ptr @"M&", !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %82)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string15)
  %83 = load double, ptr @"A#", !tbaa !2
  %84 = load double, ptr @"B#", !tbaa !2
  %85 = fcmp ogt double %83, %84
  %86 = sext i1 %85 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %86)
  %87 = load double, ptr @"B#", !tbaa !2
  %88 = load double, ptr @"A#", !tbaa !2
  %89 = fcmp ogt double %87, %88
  %90 = sext i1 %89 to i16
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %90)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string16)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sqrt.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
