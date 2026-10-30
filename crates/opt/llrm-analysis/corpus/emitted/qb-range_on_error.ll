target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [8 x i8] zeroinitializer
@$qb$statementTable = internal constant [224 x i8] c"\01\00\00\00\01\00\00\00\01\00\00\00\00\00\01\00\00\00\03\00\00\00\02\00\00\00\00\00\01\00\00\00\07\00\00\00\0D\00\00\00\00\00\01\00\00\00\0B\00\00\00\18\00\00\00\00\00\01\00\00\00\0C\00\00\00\1C\00\00\00\00\00\01\00\00\00\08\00\00\00 \00\00\00\00\00\01\00\00\00\10\00\00\00+\00\00\00\00\00\01\00\00\00\14\00\00\006\00\00\00\00\00\01\00\00\00\15\00\00\00:\00\00\00\00\00\01\00\00\00\11\00\00\00>\00\00\00\00\00\01\00\00\00\16\00\00\00@\00\00\00\00\00\01\00\00\00\19\00\00\00N\00\00\00\00\00\01\00\00\00\1A\00\00\00P\00\00\00\00\00\01\00\00\00\02\00\00\00Q\00\00\00\00\00\01\00\00\00\1C\00\00\00W\00\00\00\00\00\01\00\00\00\1D\00\00\00X\00\00\00\00\00"
@"I%" = internal global [2 x i8] zeroinitializer
@"X#" = internal global [8 x i8] zeroinitializer
@$float3 = internal constant [8 x i8] c"\00\00\00\00\00L\DD@"
@$arguments$6 = internal global [10 x i8] zeroinitializer
@$float5 = internal constant [8 x i8] c"\F5:\B7\D0,.\D6@"
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"exp\00" }>
@"Y#" = internal global [8 x i8] zeroinitializer
@$float7 = internal constant [4 x i8] zeroinitializer
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"log\00" }>
@"S!" = internal global [4 x i8] zeroinitializer
@$float9 = internal constant [4 x i8] c"\F9\02\15P"
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"sin\00" }>
@$float11 = internal constant [4 x i8] c"\00\00\00O"
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"done" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"err\00" }>
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
  br label %b3

b2:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
          to label %62 unwind label %landing

b3:
  store i16 1, ptr @"I%", !tbaa !2
  store i16 3, ptr @$data, !tbaa !2
  %0 = getelementptr i8, ptr @$data, i16 2
  store i16 1, ptr %0, !tbaa !2
  br label %b4

b4:
  %1 = getelementptr i8, ptr @$data, i16 2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = icmp sge i16 %2, 0
  br i1 %3, label %b5, label %b6

b5:
  %4 = load i16, ptr @"I%", !tbaa !2
  %5 = load i16, ptr @$data, !tbaa !2
  %6 = icmp sle i16 %4, %5
  br i1 %6, label %b7, label %b8

b6:
  %7 = load i16, ptr @"I%", !tbaa !2
  %8 = load i16, ptr @$data, !tbaa !2
  %9 = icmp sge i16 %7, %8
  br i1 %9, label %b7, label %b8

b7:
  %10 = load double, ptr @$float3, !tbaa !2
  %11 = load i16, ptr @"I%", !tbaa !2
  store i16 %11, ptr @$arguments$6, !tbaa !2
  %12 = load i16, ptr @$arguments$6, !tbaa !2
  %13 = sitofp i16 %12 to double
  %14 = fmul double %10, %13
  %15 = load double, ptr @$float5, !tbaa !2
  %16 = fcmp oge double %14, %15
  br i1 %16, label %b9, label %b10

b8:
  store i16 0, ptr @"I%", !tbaa !2
  %17 = getelementptr i8, ptr @$data, i16 4
  store i16 2, ptr %17, !tbaa !2
  %18 = getelementptr i8, ptr @$data, i16 6
  store i16 1, ptr %18, !tbaa !2
  br label %b13

b9:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %71 unwind label %landing

b10:
  %19 = call double @llvm.exp.f64(double %14)
  %20 = fpround double %19 to double
  store double %20, ptr @"X#", !tbaa !2
  br label %b11

b11:
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %68 unwind label %landing

b12:
  %21 = load i16, ptr @"I%", !tbaa !2
  %22 = getelementptr i8, ptr @$data, i16 2
  %23 = load i16, ptr %22, !tbaa !2
  %24 = add nsw i16 %21, %23
  store i16 %24, ptr @"I%", !tbaa !2
  br label %b4

b13:
  %25 = getelementptr i8, ptr @$data, i16 6
  %26 = load i16, ptr %25, !tbaa !2
  %27 = icmp sge i16 %26, 0
  br i1 %27, label %b14, label %b15

b14:
  %28 = load i16, ptr @"I%", !tbaa !2
  %29 = getelementptr i8, ptr @$data, i16 4
  %30 = load i16, ptr %29, !tbaa !2
  %31 = icmp sle i16 %28, %30
  br i1 %31, label %b16, label %b17

b15:
  %32 = load i16, ptr @"I%", !tbaa !2
  %33 = getelementptr i8, ptr @$data, i16 4
  %34 = load i16, ptr %33, !tbaa !2
  %35 = icmp sge i16 %32, %34
  br i1 %35, label %b16, label %b17

b16:
  %36 = load i16, ptr @"I%", !tbaa !2
  %37 = sub i16 %36, 1
  %38 = getelementptr i8, ptr @$arguments$6, i16 2
  store i16 %37, ptr %38, !tbaa !2
  %39 = getelementptr i8, ptr @$arguments$6, i16 2
  %40 = load i16, ptr %39, !tbaa !2
  %41 = sitofp i16 %40 to float
  %42 = load float, ptr @$float7, !tbaa !2
  %43 = fcmp ole float %41, %42
  br i1 %43, label %b18, label %b19

b17:
  %44 = load float, ptr @$float9, !tbaa !2
  store float %44, ptr @"S!", !tbaa !2
  br label %b22

b18:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %75 unwind label %landing

b19:
  %45 = call float @llvm.log.f32(float %41)
  %46 = fpext float %45 to double
  %47 = fpround double %46 to double
  store double %47, ptr @"Y#", !tbaa !2
  br label %b20

b20:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %72 unwind label %landing

b21:
  %48 = load i16, ptr @"I%", !tbaa !2
  %49 = getelementptr i8, ptr @$data, i16 6
  %50 = load i16, ptr %49, !tbaa !2
  %51 = add nsw i16 %48, %50
  store i16 %51, ptr @"I%", !tbaa !2
  br label %b13

b22:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %76 unwind label %landing

b23:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %87 unwind label %landing

b24:
  %52 = call float @llvm.sin.f32(float %82)
  %53 = fpround float %52 to float
  %54 = getelementptr i8, ptr @$arguments$6, i16 6
  store float %53, ptr %54, !tbaa !2
  %55 = getelementptr i8, ptr @$arguments$6, i16 6
  %56 = load float, ptr %55, !tbaa !2
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %56)
          to label %86 unwind label %landing

b25:
  store i16 5, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %88 unwind label %landing

b26:
  store i16 6, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$CEND()
          to label %89 unwind label %landing

b28:
  store i16 8, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 20)
          to label %90 unwind label %landing

b29:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %57 = extractvalue { ptr, i32 } %landed, 1
  %58 = trunc i32 %57 to i16
  %resumed = call addrspace(1) i16 @__main$handler(i16 %58, i16 0)
  switch i16 %resumed, label %61 [
    i16 0, label %again
    i16 1, label %next
  ]

again:
  %59 = load i16, ptr %site
  switch i16 %59, label %91 [
    i16 0, label %b7
    i16 1, label %b11
    i16 2, label %b16
    i16 3, label %b20
    i16 4, label %b22
    i16 5, label %b25
    i16 6, label %b26
    i16 7, label %b2
    i16 8, label %b28
  ]

next:
  %60 = load i16, ptr %site
  switch i16 %60, label %92 [
    i16 0, label %b11
    i16 1, label %b12
    i16 2, label %b20
    i16 3, label %b21
    i16 4, label %b25
    i16 5, label %b26
    i16 6, label %b2
    i16 7, label %b28
    i16 8, label %b29
  ]

61:
  unreachable

62:
  store i16 7, ptr %site
  %63 = invoke cc 1000 addrspace(1) i16 @llrm.qb.B$FERR()
          to label %64 unwind label %landing

64:
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %63)
          to label %65 unwind label %landing

65:
  %erl = load i16, ptr @$QB$ERL
  %66 = zext i16 %erl to i32
  store i16 7, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %66)
          to label %67 unwind label %landing

67:
  br label %b28

68:
  %69 = load i16, ptr @"I%", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %69)
          to label %70 unwind label %landing

70:
  br label %b12

71:
  unreachable

72:
  %73 = load i16, ptr @"I%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$PEI2(i16 %73)
          to label %74 unwind label %landing

74:
  br label %b21

75:
  unreachable

76:
  %77 = load float, ptr @"S!", !tbaa !2
  %78 = getelementptr i8, ptr @$arguments$6, i16 4
  store i16 1000, ptr %78, !tbaa !2
  %79 = getelementptr i8, ptr @$arguments$6, i16 4
  %80 = load i16, ptr %79, !tbaa !2
  %81 = sitofp i16 %80 to float
  %82 = fmul float %77, %81
  %83 = call float @llvm.fabs.f32(float %82)
  %84 = load float, ptr @$float11, !tbaa !2
  %85 = fcmp ogt float %83, %84
  br i1 %85, label %b23, label %b24

86:
  br label %b25

87:
  unreachable

88:
  br label %b26

89:
  unreachable

90:
  unreachable

91:
  unreachable

92:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1)

declare cc 1000 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare float @llvm.log.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

define internal i16 @__main$handler(i16 %0, i16 %1) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  store i16 %1, ptr @$QB$ERL
  %2 = load i16, ptr @$QB$ACTIVE
  switch i16 %2, label %4 [
    i16 1, label %b2
  ]

b2:
  call cc 1000 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  call cc 1000 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  %3 = zext i16 %1 to i32
  call cc 1000 addrspace(1) void @llrm.qb.B$PEI4(i32 %3)
  br label %b28

b28:
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  ret i16 1

4:
  unreachable

5:
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
