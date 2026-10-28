target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [308 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\05\00\00\00\02\00\00\00\00\00\01\00\00\00\06\00\00\00\03\00\00\00\00\00\01\00\00\00\0A\00\00\00\0E\00\00\00\00\00\01\00\00\00\0C\00\00\00\10\00\00\00\00\00\01\00\00\00\0D\00\00\00\15\00\00\00\00\00\01\00\00\00\0E\00\00\00\19\00\00\00\00\00\01\00\00\00\11\00\00\00 \00\00\00\00\00\01\00\00\00\12\00\00\00\22\00\00\00\00\00\01\00\00\00\13\00\00\00$\00\00\00\00\00\01\00\00\00\0B\00\00\00(\00\00\00\00\00\01\00\00\00\14\00\00\00)\00\00\00\00\00\01\00\00\00\18\00\00\004\00\00\00\00\00\01\00\00\00\1A\00\00\006\00\00\00\00\00\01\00\00\00\1B\00\00\00:\00\00\00\00\00\01\00\00\00\1E\00\00\00B\00\00\00\00\00\01\00\00\00\1F\00\00\00D\00\00\00\00\00\01\00\00\00 \00\00\00F\00\00\00\00\00\01\00\00\00\19\00\00\00J\00\00\00\00\00\01\00\00\00\02\00\00\00K\00\00\00\00\00\01\00\00\00\22\00\00\00O\00\00\00\00\00\01\00\00\00$\00\00\00R\00\00\00\00\00"
@"X#" = internal global [8 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"Z#" = internal global [8 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$arguments$7 = internal global [14 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] c"\F5:\B7\D0,.\D6@"
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"ok" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c".\00" }>
@$float7 = internal constant [4 x i8] c"gq\B1F"
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"ok" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c".\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"error\00" }>
@$qb$readData = internal constant [91 x i8] c" 22712.6, 22712.8, 23000, 100000, -23000, -100000, 0, 1E+300\00 22712, 22713, -22713, 100000\00"
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
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %60 unwind label %landing

b3:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:0"()
          to label %64 unwind label %landing

b4:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$DATA:1"()
          to label %65 unwind label %landing

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$RSTB:0"(i16 0)
          to label %66 unwind label %landing

b6:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 8, ptr @$data, !tbaa !2
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
          to label %68 unwind label %landing

b11:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @"llrm.qb.$QB$RSTB:1"(i16 0)
          to label %67 unwind label %landing

b12:
  store i16 -1, ptr @$arguments$7, !tbaa !2
  %11 = load i16, ptr @$arguments$7, !tbaa !2
  %12 = sitofp i16 %11 to double
  %13 = fpround double %12 to double
  store double %13, ptr @"Z#", !tbaa !2
  br label %b13

b13:
  %14 = load double, ptr @"X#", !tbaa !2
  %15 = getelementptr i8, ptr @$arguments$7, i16 2
  store double %14, ptr %15, !tbaa !2
  %16 = getelementptr i8, ptr @$arguments$7, i16 2
  %17 = load double, ptr %16, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR8(double %17)
          to label %69 unwind label %landing

b14:
  %18 = load double, ptr @"X#", !tbaa !2
  %19 = load double, ptr @$float4, !tbaa !2
  %20 = fcmp oge double %18, %19
  br i1 %20, label %b15, label %b16

b15:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %73 unwind label %landing

b16:
  %21 = call double @llvm.exp.f64(double %18)
  %22 = fpround double %21 to double
  store double %22, ptr @"Z#", !tbaa !2
  br label %b17

b17:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %71 unwind label %landing

b18:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %72 unwind label %landing

b19:
  %23 = load i16, ptr @"K%", !tbaa !2
  %24 = getelementptr i8, ptr @$data, i16 2
  %25 = load i16, ptr %24, !tbaa !2
  %26 = add nsw i16 %23, %25
  store i16 %26, ptr @"K%", !tbaa !2
  br label %b7

b20:
  store i16 1, ptr @"K%", !tbaa !2
  %27 = getelementptr i8, ptr @$data, i16 4
  store i16 4, ptr %27, !tbaa !2
  %28 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %28, !tbaa !2
  br label %b21

b21:
  %29 = getelementptr i8, ptr @$data, i16 6
  %30 = load i16, ptr %29, !tbaa !2
  %31 = icmp sge i16 %30, 0
  br i1 %31, label %b22, label %b23

b22:
  %32 = load i16, ptr @"K%", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 4
  %34 = load i16, ptr %33, !tbaa !2
  %35 = icmp sle i16 %32, %34
  br i1 %35, label %b24, label %b25

b23:
  %36 = load i16, ptr @"K%", !tbaa !2
  %37 = getelementptr i8, ptr @$data, i16 4
  %38 = load i16, ptr %37, !tbaa !2
  %39 = icmp sge i16 %36, %38
  br i1 %39, label %b24, label %b25

b24:
  %40 = addrspacecast ptr @"S!" to ptr addrspace(1)
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) %40)
          to label %75 unwind label %landing

b25:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %74 unwind label %landing

b26:
  %41 = load float, ptr @"S!", !tbaa !2
  %42 = getelementptr i8, ptr @$arguments$7, i16 10
  store float %41, ptr %42, !tbaa !2
  %43 = getelementptr i8, ptr @$arguments$7, i16 10
  %44 = load float, ptr %43, !tbaa !2
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSR4(float %44)
          to label %76 unwind label %landing

b27:
  %45 = load float, ptr @"S!", !tbaa !2
  %46 = load float, ptr @$float7, !tbaa !2
  %47 = fcmp oge float %45, %46
  br i1 %47, label %b28, label %b29

b28:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %80 unwind label %landing

b29:
  %48 = call float @llvm.exp.f32(float %45)
  %49 = fpext float %48 to double
  %50 = fpround double %49 to double
  store double %50, ptr @"Z#", !tbaa !2
  br label %b30

b30:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
          to label %78 unwind label %landing

b31:
  store i16 11, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string9)
          to label %79 unwind label %landing

b32:
  %51 = load i16, ptr @"K%", !tbaa !2
  %52 = getelementptr i8, ptr @$data, i16 6
  %53 = load i16, ptr %52, !tbaa !2
  %54 = add nsw i16 %51, %53
  store i16 %54, ptr @"K%", !tbaa !2
  br label %b21

b34:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %81 unwind label %landing

b36:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %55 = extractvalue { ptr, i32 } %landed, 1
  %56 = trunc i32 %55 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %56, i16 0)
  switch i16 %resumed, label %59 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %57 = load i16, ptr %site
  switch i16 %57, label %82 [
    i16 0, label %b5
    i16 1, label %b10
    i16 2, label %b13
    i16 3, label %b14
    i16 4, label %b17
    i16 5, label %b18
    i16 6, label %b11
    i16 7, label %b24
    i16 8, label %b26
    i16 9, label %b27
    i16 10, label %b30
    i16 11, label %b31
    i16 12, label %b25
    i16 13, label %b2
    i16 14, label %b34
  ]

next:
  %58 = load i16, ptr %site
  switch i16 %58, label %83 [
    i16 0, label %b6
    i16 1, label %b12
    i16 2, label %b14
    i16 3, label %b17
    i16 4, label %b18
    i16 5, label %b19
    i16 6, label %b20
    i16 7, label %b26
    i16 8, label %b27
    i16 9, label %b30
    i16 10, label %b31
    i16 11, label %b32
    i16 12, label %b2
    i16 13, label %b34
    i16 14, label %b36
  ]

59:
  unreachable

60:
  store i16 13, ptr %site
  %61 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %62 unwind label %landing

62:
  store i16 13, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %61)
          to label %63 unwind label %landing

63:
  br label %b34

64:
  br label %b4

65:
  br label %b36

66:
  br label %b6

67:
  br label %b20

68:
  br label %b12

69:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %70 unwind label %landing

70:
  br label %b14

71:
  br label %b18

72:
  br label %b19

73:
  unreachable

74:
  unreachable

75:
  br label %b26

76:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEOS()
          to label %77 unwind label %landing

77:
  br label %b27

78:
  br label %b31

79:
  br label %b32

80:
  unreachable

81:
  unreachable

82:
  unreachable

83:
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

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$RDR4(ptr addrspace(1)) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSR4(float) addrspace(1)

declare float @llvm.exp.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %3 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %0)
  br label %b34

b34:
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
