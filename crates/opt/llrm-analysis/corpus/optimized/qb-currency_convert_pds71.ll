target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A@" = internal global [8 x i8] zeroinitializer
@"C@" = internal global [8 x i8] zeroinitializer
@"I%" = internal global [2 x i8] zeroinitializer
@"L&" = internal global [4 x i8] zeroinitializer
@"D#" = internal global [8 x i8] zeroinitializer
@"N!" = internal global [4 x i8] zeroinitializer
@"X#" = internal global [8 x i8] zeroinitializer
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"=>" }>
@$string9 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string9, i16 4), [4 x i8] c" i=\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c" l=\00" }>
@$string17 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string17, i16 4), [4 x i8] c"err\00" }>

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store float 1.000000e+00, ptr @"N!", !tbaa !2
  %0 = tail call i64 @llvm.lrint.i64.f80(x86_fp80 0xK403E8000000000001060)
  %1 = tail call i64 @llvm.lrint.i64.f80(x86_fp80 0xKC03E80000000000001BA)
  %2 = tail call i64 @llvm.lrint.i64.f80(x86_fp80 0xK406FF684DF56C3E01D28)
  %3 = fdiv double 0x4206FEDFF2100000, 1.000000e+04
  %4 = fpround double %3 to double
  %5 = fdiv double 0x43E0000000000000, 1.000000e+04
  %6 = fpround double %5 to double
  %7 = fdiv double 1.000000e+00, 1.000000e+04
  %8 = fpround double %7 to double
  br label %b6

b6:
  %9 = phi float [ %25, %b17 ], [ 1.000000e+00, %frame ]
  %10 = fcmp ole float %9, 1.800000e+01
  br i1 %10, label %b9, label %b10

b9:
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
          to label %b11 unwind label %landing

b10:
  store double 0x430A36E2EB1C432D, ptr @"D#", !tbaa !2
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER8(double 0x430A36E2EB1C432D)
          to label %b19 unwind label %landing

b11:
  %11 = load double, ptr @"X#", !tbaa !2
  %12 = fpext double %11 to x86_fp80
  %13 = fmul x86_fp80 %12, 1.000000e+04
  %14 = call i64 @llvm.lrint.i64.f80(x86_fp80 %13)
  store i64 %14, ptr @"C@", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %11)
          to label %62 unwind label %landing

b13:
  %15 = load i64, ptr @"C@", !tbaa !2
  %16 = sitofp i64 %15 to double
  %17 = fdiv double %16, 1.000000e+04
  %18 = call i16 @llvm.lrint.i16.f64(double %17)
  store i16 %18, ptr @"I%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %66 unwind label %landing

b15:
  %19 = load i64, ptr @"C@", !tbaa !2
  %20 = sitofp i64 %19 to double
  %21 = fdiv double %20, 1.000000e+04
  %22 = call i32 @llvm.lrint.i32.f64(double %21)
  store i32 %22, ptr @"L&", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %69 unwind label %landing

b17:
  %23 = load float, ptr @"N!", !tbaa !2
  %24 = fadd float %23, 1.000000e+00
  %25 = fpround float %24 to float
  store float %25, ptr @"N!", !tbaa !2
  br label %b6

b19:
  %26 = load double, ptr @"D#", !tbaa !2
  %27 = fpext double %26 to x86_fp80
  %28 = fmul x86_fp80 %27, 1.000000e+04
  %29 = call i64 @llvm.lrint.i64.f80(x86_fp80 %28)
  store i64 %29, ptr @"C@", !tbaa !2
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %29)
          to label %b21 unwind label %landing

b21:
  store double 0x430A36E2EB1C4330, ptr @"D#", !tbaa !2
  store i64 %0, ptr @"C@", !tbaa !2
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %0)
          to label %b24 unwind label %landing

b24:
  store double 0xC30A36E2EB1C432D, ptr @"D#", !tbaa !2
  store i64 %1, ptr @"C@", !tbaa !2
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %1)
          to label %b27 unwind label %landing

b27:
  store double 1.500000e-04, ptr @"D#", !tbaa !2
  store i64 1, ptr @"C@", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 1)
          to label %b30 unwind label %landing

b30:
  store double 1.000000e+30, ptr @"D#", !tbaa !2
  store i64 %2, ptr @"C@", !tbaa !2
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 %2)
          to label %b33 unwind label %landing

b33:
  store i64 0, ptr @"C@", !tbaa !2
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 0)
          to label %b36 unwind label %landing

b36:
  store i64 167772160000, ptr @"C@", !tbaa !2
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 167772160000)
          to label %b39 unwind label %landing

b39:
  store i64 12345671234, ptr @"A@", !tbaa !2
  store double %4, ptr @"D#", !tbaa !2
  store i16 12, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %4)
          to label %b42 unwind label %landing

b42:
  %30 = load i64, ptr @"A@", !tbaa !2
  %31 = sitofp i64 %30 to double
  %32 = fdiv double %31, 1.000000e+04
  %33 = fptrunc double %32 to float
  %34 = fpround float %33 to float
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER4(float %34)
          to label %b44 unwind label %landing

b44:
  store i64 9223372036854775807, ptr @"A@", !tbaa !2
  store double %6, ptr @"D#", !tbaa !2
  store i16 14, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %6)
          to label %b47 unwind label %landing

b47:
  %35 = load i64, ptr @"A@", !tbaa !2
  %36 = sitofp i64 %35 to double
  %37 = fdiv double %36, 1.000000e+04
  %38 = fptrunc double %37 to float
  %39 = fpround float %38 to float
  store i16 15, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER4(float %39)
          to label %b49 unwind label %landing

b49:
  store i64 1, ptr @"A@", !tbaa !2
  store double %8, ptr @"D#", !tbaa !2
  store i16 16, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER8(double %8)
          to label %b52 unwind label %landing

b52:
  store i16 -32768, ptr @"I%", !tbaa !2
  store i64 -327680000, ptr @"C@", !tbaa !2
  store i16 17, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 -327680000)
          to label %b55 unwind label %landing

b55:
  store i32 -2147483647, ptr @"L&", !tbaa !2
  store i64 -21474836470000, ptr @"C@", !tbaa !2
  store i16 18, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PECY(i64 -21474836470000)
          to label %b58 unwind label %landing

b58:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %40 = extractvalue { ptr, i32 } %landed, 1
  %41 = trunc i32 %40 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %41)
  %42 = load i16, ptr %site
  switch table i16 %42, label %43 [
    i16 0, label %43
    i16 1, label %44
    i16 2, label %45
    i16 3, label %46
    i16 4, label %47
    i16 5, label %48
    i16 6, label %49
    i16 7, label %50
    i16 8, label %51
    i16 9, label %52
    i16 10, label %53
    i16 11, label %54
    i16 12, label %55
    i16 13, label %56
    i16 14, label %57
    i16 15, label %58
    i16 16, label %59
    i16 17, label %60
    i16 18, label %61
  ]

43:
  br label %b11

44:
  br label %b13

45:
  br label %b15

46:
  br label %b17

47:
  br label %b19

48:
  br label %b21

49:
  br label %b24

50:
  br label %b27

51:
  br label %b30

52:
  br label %b33

53:
  br label %b36

54:
  br label %b39

55:
  br label %b42

56:
  br label %b44

57:
  br label %b47

58:
  br label %b49

59:
  br label %b52

60:
  br label %b55

61:
  br label %b58

62:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %63 unwind label %landing

63:
  %64 = load i64, ptr @"C@", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSCY(i64 %64)
          to label %65 unwind label %landing

65:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b13 unwind label %landing

66:
  %67 = load i16, ptr @"I%", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %67)
          to label %68 unwind label %landing

68:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b15 unwind label %landing

69:
  %70 = load i32, ptr @"L&", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 %70)
          to label %b17 unwind label %landing
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$RDR8(ptr addrspace(1) nocapture noretain) addrspace(1)

declare i64 @llvm.lrint.i64.f80(x86_fp80) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSCY(i64) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEOS() addrspace(1)

declare i16 @llvm.lrint.i16.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare cc 1005 void @llrm.qb.B$PER8(double) addrspace(1)

declare cc 1005 void @llrm.qb.B$PECY(i64) addrspace(1)

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string17)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  tail call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 poison
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
