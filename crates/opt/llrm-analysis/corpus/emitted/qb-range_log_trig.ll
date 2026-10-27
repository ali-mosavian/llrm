target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [378 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\05\00\00\00\02\00\00\00\00\00\01\00\00\00\06\00\00\00\03\00\00\00\00\00\01\00\00\00\0A\00\00\00\0E\00\00\00\00\00\01\00\00\00\0C\00\00\00\10\00\00\00\00\00\01\00\00\00\0D\00\00\00\14\00\00\00\00\00\01\00\00\00\10\00\00\00\1B\00\00\00\00\00\01\00\00\00\11\00\00\00\1E\00\00\00\00\00\01\00\00\00\12\00\00\00\22\00\00\00\00\00\01\00\00\00\13\00\00\00$\00\00\00\00\00\01\00\00\00\14\00\00\00&\00\00\00\00\00\01\00\00\00\0B\00\00\00*\00\00\00\00\00\01\00\00\00\15\00\00\00+\00\00\00\00\00\01\00\00\00\19\00\00\006\00\00\00\00\00\01\00\00\00\1B\00\00\008\00\00\00\00\00\01\00\00\00\1C\00\00\00<\00\00\00\00\00\01\00\00\00\1F\00\00\00D\00\00\00\00\00\01\00\00\00 \00\00\00G\00\00\00\00\00\01\00\00\00#\00\00\00O\00\00\00\00\00\01\00\00\00$\00\00\00R\00\00\00\00\00\01\00\00\00'\00\00\00\\\00\00\00\00\00\01\00\00\00(\00\00\00^\00\00\00\00\00\01\00\00\00)\00\00\00`\00\00\00\00\00\01\00\00\00\1A\00\00\00d\00\00\00\00\00\01\00\00\00\02\00\00\00e\00\00\00\00\00\01\00\00\00+\00\00\00i\00\00\00\00\00\01\00\00\00-\00\00\00l\00\00\00\00\00"
@"X#" = internal global [8 x i8] zeroinitializer
@"Z#" = internal global [8 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$arguments$6 = internal global [16 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"log ok" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"sqr ok" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c".\00" }>
@$float8 = internal constant [8 x i8] c"\00\00\00\00\00\00\E0A"
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"sin ok" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"cos ok" }>
@$string11 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string11, i16 4), [6 x i8] c"tan ok" }>
@$string12 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string12, i16 4), [2 x i8] c".\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string13, i16 4), [6 x i8] c"error\00" }>
@$qb$readData = internal constant [112 x i8] c" 0, -1, -1E-300, 1E-300, 1, 1E+300\00 100, 2147483648, 2147483648.5, 2147483649, -2147483648, -2147483649, 1E+300\00"
@$QB$ACTIVE = internal global i16 0
@$QB$ERL = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr %site)
  br label %b1

b1:
  store i16 1, ptr @$QB$ACTIVE
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  br label %b5

b2:
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
          to label %72 unwind label %landing

b3:
  store i16 19, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
          to label %76 unwind label %landing

b4:
  store i16 19, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:1"()
          to label %77 unwind label %landing

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$RSTB:0"(i16 0)
          to label %78 unwind label %landing

b6:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 6, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b7

b7:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b8, label %b9

b8:
  %4 = load i16, ptr @"K%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b10, label %b11

b9:
  %7 = load i16, ptr @"K%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b10, label %b11

b10:
  %10 = addrspacecast ptr @"X#" to ptr addrspace(1)
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %10)
          to label %80 unwind label %landing

b11:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$RSTB:1"(i16 0)
          to label %79 unwind label %landing

b12:
  %11 = load double, ptr @"X#", !tbaa !2
  store double %11, ptr @$arguments$6, !tbaa !2
  %12 = load double, ptr @$arguments$6, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %12)
          to label %81 unwind label %landing

b13:
  %13 = load double, ptr @"X#", !tbaa !2
  %14 = load double, ptr @$float4, !tbaa !2
  %15 = fcmp ole double %13, %14
  br i1 %15, label %b14, label %b15

b14:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %87 unwind label %landing

b15:
  %16 = call double @llvm.log.f64(double %13)
  %17 = fpround double %16 to double
  store double %17, ptr @"Z#", !tbaa !2
  br label %b16

b16:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %83 unwind label %landing

b17:
  %18 = load double, ptr @"X#", !tbaa !2
  %19 = call double @llvm.sqrt.f64(double %18)
  %20 = fpround double %19 to double
  store double %20, ptr @"Z#", !tbaa !2
  br label %b18

b18:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %85 unwind label %landing

b19:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %86 unwind label %landing

b20:
  %21 = load i16, ptr @"K%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add nsw i16 %21, %23
  store i16 %24, ptr @"K%", !tbaa !2
  br label %b7

b21:
  store i16 1, ptr @"K%", !tbaa !2
  %25 = getelementptr i8, ptr @$data, i16 4
  store i16 7, ptr %25, !tbaa !2
  %26 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %26, !tbaa !2
  br label %b22

b22:
  %27 = getelementptr i8, ptr @$data, i16 6
  %28 = load i16, ptr %27, !tbaa !2
  %29 = icmp sge i16 %28, 0
  br i1 %29, label %b23, label %b24

b23:
  %30 = load i16, ptr @"K%", !tbaa !2
  %31 = getelementptr i8, ptr @$data, i16 4
  %32 = load i16, ptr %31, !tbaa !2
  %33 = icmp sle i16 %30, %32
  br i1 %33, label %b25, label %b26

b24:
  %34 = load i16, ptr @"K%", !tbaa !2
  %35 = getelementptr i8, ptr @$data, i16 4
  %36 = load i16, ptr %35, !tbaa !2
  %37 = icmp sge i16 %34, %36
  br i1 %37, label %b25, label %b26

b25:
  %38 = addrspacecast ptr @"X#" to ptr addrspace(1)
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %38)
          to label %89 unwind label %landing

b26:
  store i16 17, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %88 unwind label %landing

b27:
  %39 = load double, ptr @"X#", !tbaa !2
  %40 = getelementptr i8, ptr @$arguments$6, i16 8
  store double %39, ptr %40, !tbaa !2
  %41 = getelementptr i8, ptr @$arguments$6, i16 8
  %42 = load double, ptr %41, !tbaa !2
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %42)
          to label %90 unwind label %landing

b28:
  %43 = load double, ptr @"X#", !tbaa !2
  %44 = call double @llvm.fabs.f64(double %43)
  %45 = load double, ptr @$float8, !tbaa !2
  %46 = fcmp ogt double %44, %45
  br i1 %46, label %b29, label %b30

b29:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %100 unwind label %landing

b30:
  %47 = call double @llvm.sin.f64(double %43)
  %48 = fpround double %47 to double
  store double %48, ptr @"Z#", !tbaa !2
  br label %b31

b31:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %92 unwind label %landing

b32:
  %49 = load double, ptr @"X#", !tbaa !2
  %50 = call double @llvm.fabs.f64(double %49)
  %51 = load double, ptr @$float8, !tbaa !2
  %52 = fcmp ogt double %50, %51
  br i1 %52, label %b33, label %b34

b33:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %99 unwind label %landing

b34:
  %53 = call double @llvm.cos.f64(double %49)
  %54 = fpround double %53 to double
  store double %54, ptr @"Z#", !tbaa !2
  br label %b35

b35:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %94 unwind label %landing

b36:
  %55 = load double, ptr @"X#", !tbaa !2
  %56 = call double @llvm.fabs.f64(double %55)
  %57 = load double, ptr @$float8, !tbaa !2
  %58 = fcmp ogt double %56, %57
  br i1 %58, label %b37, label %b38

b37:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %98 unwind label %landing

b38:
  %59 = call double @llvm.sin.f64(double %55)
  %60 = call double @llvm.cos.f64(double %55)
  %61 = fdiv double %59, %60
  %62 = fpround double %61 to double
  store double %62, ptr @"Z#", !tbaa !2
  br label %b39

b39:
  store i16 15, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
          to label %96 unwind label %landing

b40:
  store i16 16, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %97 unwind label %landing

b41:
  %63 = load i16, ptr @"K%", !tbaa !2
  %64 = getelementptr i8, ptr @$data, i16 6
  %65 = load i16, ptr %64, !tbaa !2
  %66 = add nsw i16 %63, %65
  store i16 %66, ptr @"K%", !tbaa !2
  br label %b22

b43:
  store i16 19, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %101 unwind label %landing

b45:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %67 = extractvalue { ptr, i32 } %landed, 1
  %68 = trunc i32 %67 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %68, i16 0)
  switch i16 %resumed, label %71 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %69 = load i16, ptr %site
  switch i16 %69, label %102 [
    i16 0, label %b5
    i16 1, label %b10
    i16 2, label %b12
    i16 3, label %b13
    i16 4, label %b16
    i16 5, label %b18
    i16 6, label %b19
    i16 7, label %b11
    i16 8, label %b25
    i16 9, label %b27
    i16 10, label %b28
    i16 11, label %b31
    i16 12, label %b32
    i16 13, label %b35
    i16 14, label %b36
    i16 15, label %b39
    i16 16, label %b40
    i16 17, label %b26
    i16 18, label %b2
    i16 19, label %b43
  ]

next:
  %70 = load i16, ptr %site
  switch i16 %70, label %103 [
    i16 0, label %b6
    i16 1, label %b12
    i16 2, label %b13
    i16 3, label %b16
    i16 4, label %b17
    i16 5, label %b19
    i16 6, label %b20
    i16 7, label %b21
    i16 8, label %b27
    i16 9, label %b28
    i16 10, label %b31
    i16 11, label %b32
    i16 12, label %b35
    i16 13, label %b36
    i16 14, label %b39
    i16 15, label %b40
    i16 16, label %b41
    i16 17, label %b2
    i16 18, label %b43
    i16 19, label %b45
  ]

71:
  unreachable

72:
  store i16 18, ptr %site
  %73 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %74 unwind label %landing

74:
  store i16 18, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %73)
          to label %75 unwind label %landing

75:
  br label %b43

76:
  br label %b4

77:
  br label %b45

78:
  br label %b6

79:
  br label %b21

80:
  br label %b12

81:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %82 unwind label %landing

82:
  br label %b13

83:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %84 unwind label %landing

84:
  br label %b17

85:
  br label %b19

86:
  br label %b20

87:
  unreachable

88:
  unreachable

89:
  br label %b27

90:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %91 unwind label %landing

91:
  br label %b28

92:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %93 unwind label %landing

93:
  br label %b32

94:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %95 unwind label %landing

95:
  br label %b36

96:
  br label %b40

97:
  br label %b41

98:
  unreachable

99:
  unreachable

100:
  unreachable

101:
  unreachable

102:
  unreachable

103:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$DATA:1"() addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$RSTB:0"(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR8(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$RSTB:1"(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEOS() addrspace(1)

declare double @llvm.log.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sqrt.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare double @llvm.fabs.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sin.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b43

b43:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

3:
  unreachable

4:
  unreachable
}

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
