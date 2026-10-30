target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [4 x i8] zeroinitializer
@$qb$statementTable = internal constant [182 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\04\00\00\00\02\00\00\00\00\00\01\00\00\00\05\00\00\00\03\00\00\00\00\00\01\00\00\00\09\00\00\00\0E\00\00\00\00\00\01\00\00\00\0B\00\00\00\12\00\00\00\00\00\01\00\00\00\0C\00\00\00\19\00\00\00\00\00\01\00\00\00\1C\00\00\00T\00\00\00\00\00\01\00\00\00\1D\00\00\00V\00\00\00\00\00\01\00\00\00\1E\00\00\00X\00\00\00\00\00\01\00\00\00\0A\00\00\00\\\00\00\00\00\00\01\00\00\00\02\00\00\00]\00\00\00\00\00\01\00\00\00 \00\00\00a\00\00\00\00\00\01\00\00\00\22\00\00\00c\00\00\00\00\00"
@"X#" = internal global [8 x i8] zeroinitializer
@"Y#" = internal global [8 x i8] zeroinitializer
@"Z#" = internal global [8 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$arguments$7 = internal global [48 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] zeroinitializer
@$float5 = internal constant [8 x i8] c"\00\00\00\00\E0\FF\DF@"
@$float6 = internal constant [8 x i8] c"\00\00\00\00\00\00\F0?"
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"ok" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c".\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"error\00" }>
@$qb$readData = internal constant [109 x i8] c" 2, 100000, 10, 22712, 10, 500, 1E+200, 2, 10, -400, 0, -1, -8, .3333333333333333, -2, 3, 2, 32767, 2, 32768\00"
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
  br label %b4

b2:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %93 unwind label %landing

b3:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
          to label %97 unwind label %landing

b4:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$RSTB:0"(i16 0)
          to label %98 unwind label %landing

b5:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 10, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b6

b6:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b7, label %b8

b7:
  %4 = load i16, ptr @"K%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b9, label %b10

b8:
  %7 = load i16, ptr @"K%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b9, label %b10

b9:
  %10 = addrspacecast ptr @"X#" to ptr addrspace(1)
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %10)
          to label %100 unwind label %landing

b10:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %99 unwind label %landing

b11:
  %11 = load double, ptr @"X#", !tbaa !2
  store double %11, ptr @$arguments$7, !tbaa !2
  %12 = load double, ptr @$arguments$7, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %12)
          to label %103 unwind label %landing

b12:
  %13 = load double, ptr @"X#", !tbaa !2
  %14 = load double, ptr @"Y#", !tbaa !2
  %15 = getelementptr i8, ptr @$arguments$7, i16 16
  store double %13, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$arguments$7, i16 24
  store double %14, ptr %16, !tbaa !2
  %17 = getelementptr i8, ptr @$arguments$7, i16 16
  %18 = load double, ptr %17, !tbaa !2
  %19 = load double, ptr @$float4, !tbaa !2
  %20 = fcmp ogt double %18, %19
  br i1 %20, label %b13, label %b14

b13:
  %21 = getelementptr i8, ptr @$arguments$7, i16 16
  %22 = load double, ptr %21, !tbaa !2
  %23 = call double @llvm.log2.f64(double %22)
  %24 = getelementptr i8, ptr @$arguments$7, i16 24
  %25 = load double, ptr %24, !tbaa !2
  %26 = fmul double %23, %25
  %27 = load double, ptr @$float5, !tbaa !2
  %28 = fcmp oge double %26, %27
  br i1 %28, label %b22, label %b23

b14:
  %29 = getelementptr i8, ptr @$arguments$7, i16 16
  %30 = load double, ptr %29, !tbaa !2
  %31 = load double, ptr @$float4, !tbaa !2
  %32 = fcmp oeq double %30, %31
  br i1 %32, label %b15, label %b17

b15:
  %33 = getelementptr i8, ptr @$arguments$7, i16 24
  %34 = load double, ptr %33, !tbaa !2
  %35 = load double, ptr @$float4, !tbaa !2
  %36 = fcmp ogt double %34, %35
  br i1 %36, label %b24, label %b16

b16:
  %37 = getelementptr i8, ptr @$arguments$7, i16 24
  %38 = load double, ptr %37, !tbaa !2
  %39 = load double, ptr @$float4, !tbaa !2
  %40 = fcmp oeq double %38, %39
  br i1 %40, label %b25, label %b20

b17:
  %41 = getelementptr i8, ptr @$arguments$7, i16 24
  %42 = load double, ptr %41, !tbaa !2
  %43 = call i32 @llvm.lrint.i32.f64(double %42)
  %44 = getelementptr i8, ptr @$arguments$7, i16 40
  store i32 %43, ptr %44, !tbaa !2
  %45 = getelementptr i8, ptr @$arguments$7, i16 40
  %46 = load i32, ptr %45, !tbaa !2
  %47 = getelementptr i8, ptr @$arguments$7, i16 44
  store i32 %46, ptr %47, !tbaa !2
  %48 = getelementptr i8, ptr @$arguments$7, i16 44
  %49 = load i32, ptr %48, !tbaa !2
  %50 = sitofp i32 %49 to double
  %51 = getelementptr i8, ptr @$arguments$7, i16 24
  %52 = load double, ptr %51, !tbaa !2
  %53 = fcmp oeq double %50, %52
  br i1 %53, label %b18, label %b20

b18:
  %54 = getelementptr i8, ptr @$arguments$7, i16 16
  %55 = load double, ptr %54, !tbaa !2
  %56 = fneg double %55
  %57 = call double @llvm.log2.f64(double %56)
  %58 = getelementptr i8, ptr @$arguments$7, i16 24
  %59 = load double, ptr %58, !tbaa !2
  %60 = fmul double %57, %59
  %61 = load double, ptr @$float5, !tbaa !2
  %62 = fcmp oge double %60, %61
  br i1 %62, label %b26, label %b27

b19:
  %63 = getelementptr i8, ptr @$arguments$7, i16 32
  %64 = load double, ptr %63, !tbaa !2
  %65 = fneg double %64
  %66 = fpround double %65 to double
  %67 = getelementptr i8, ptr @$arguments$7, i16 32
  store double %66, ptr %67, !tbaa !2
  br label %b21

b20:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %111 unwind label %landing

b21:
  %68 = getelementptr i8, ptr @$arguments$7, i16 32
  %69 = load double, ptr %68, !tbaa !2
  store double %69, ptr @"Z#", !tbaa !2
  br label %b28

b22:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %114 unwind label %landing

b23:
  %70 = call double @llvm.exp2.f64(double %26)
  %71 = fpround double %70 to double
  %72 = getelementptr i8, ptr @$arguments$7, i16 32
  store double %71, ptr %72, !tbaa !2
  br label %b21

b24:
  %73 = load double, ptr @$float4, !tbaa !2
  %74 = getelementptr i8, ptr @$arguments$7, i16 32
  store double %73, ptr %74, !tbaa !2
  br label %b21

b25:
  %75 = load double, ptr @$float6, !tbaa !2
  %76 = getelementptr i8, ptr @$arguments$7, i16 32
  store double %75, ptr %76, !tbaa !2
  br label %b21

b26:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %110 unwind label %landing

b27:
  %77 = call double @llvm.exp2.f64(double %60)
  %78 = fpround double %77 to double
  %79 = getelementptr i8, ptr @$arguments$7, i16 32
  store double %78, ptr %79, !tbaa !2
  %80 = getelementptr i8, ptr @$arguments$7, i16 40
  %81 = load i32, ptr %80, !tbaa !2
  %82 = and i32 %81, 1
  %83 = icmp ne i32 %82, 0
  br i1 %83, label %b19, label %b21

b28:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %112 unwind label %landing

b29:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
          to label %113 unwind label %landing

b30:
  %84 = load i16, ptr @"K%", !tbaa !2
  %85 = getelementptr i8, ptr @$data, i16 2
  %86 = load i16, ptr %85, !tbaa !2
  %87 = add nsw i16 %84, %86
  store i16 %87, ptr @"K%", !tbaa !2
  br label %b6

b32:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %115 unwind label %landing

b34:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %88 = extractvalue { ptr, i32 } %landed, 1
  %89 = trunc i32 %88 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %89, i16 0)
  switch i16 %resumed, label %92 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %90 = load i16, ptr %site
  switch i16 %90, label %116 [
    i16 0, label %b4
    i16 1, label %b9
    i16 2, label %b11
    i16 3, label %b12
    i16 4, label %b28
    i16 5, label %b29
    i16 6, label %b10
    i16 7, label %b2
    i16 8, label %b32
  ]

next:
  %91 = load i16, ptr %site
  switch i16 %91, label %117 [
    i16 0, label %b5
    i16 1, label %b11
    i16 2, label %b12
    i16 3, label %b28
    i16 4, label %b29
    i16 5, label %b30
    i16 6, label %b2
    i16 7, label %b32
    i16 8, label %b34
  ]

92:
  unreachable

93:
  store i16 7, ptr %site
  %94 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %95 unwind label %landing

95:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %94)
          to label %96 unwind label %landing

96:
  br label %b32

97:
  br label %b34

98:
  br label %b5

99:
  unreachable

100:
  %101 = addrspacecast ptr @"Y#" to ptr addrspace(1)
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) %101)
          to label %102 unwind label %landing

102:
  br label %b11

103:
  %104 = load double, ptr @"Y#", !tbaa !2
  %105 = getelementptr i8, ptr @$arguments$7, i16 8
  store double %104, ptr %105, !tbaa !2
  %106 = getelementptr i8, ptr @$arguments$7, i16 8
  %107 = load double, ptr %106, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %107)
          to label %108 unwind label %landing

108:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %109 unwind label %landing

109:
  br label %b12

110:
  unreachable

111:
  unreachable

112:
  br label %b29

113:
  br label %b30

114:
  unreachable

115:
  unreachable

116:
  unreachable

117:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$DATA:0"() addrspace(1)

declare cc 1000 void @"llrm.qb.$QB$RSTB:0"(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR8(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEOS() addrspace(1)

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b32

b32:
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
