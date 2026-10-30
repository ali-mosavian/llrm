target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X#" = internal global [8 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"log ok" }>
@$string6 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string6, i16 4), [6 x i8] c"sqr ok" }>
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c".\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"sin ok" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"cos ok" }>
@$string11 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\06\00", ptr getelementptr (i8, ptr @$string11, i16 4), [6 x i8] c"tan ok" }>
@$string12 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string12, i16 4), [2 x i8] c".\00" }>
@$string13 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string13, i16 4), [6 x i8] c"error\00" }>
@$data.0 = internal global i16 0
@$data.1 = internal global i16 0
@$data.2 = internal global i16 0
@$data.3 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RSTB(i16 0)
          to label %b6 unwind label %landing

b6:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 6, ptr @$data.0, !tbaa !2
  store i16 1, ptr @$data.1, !tbaa !2
  br label %b7

b7:
  %0 = phi i16 [ %11, %b20 ], [ 1, %b6 ]
  %1 = phi i16 [ %10, %b20 ], [ 1, %b6 ]
  %2 = icmp sgt i16 %1, -1
  %3 = load i16, ptr @$data.0, !tbaa !2
  br i1 %2, label %b8, label %b9

b8:
  %4 = icmp sgt i16 %0, %3
  br i1 %4, label %b11, label %b10

b9:
  %5 = icmp slt i16 %0, %3
  br i1 %5, label %b11, label %b10

b10:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
          to label %b12 unwind label %landing

b11:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RSTB(i16 1)
          to label %b21 unwind label %landing

b12:
  %6 = load double, ptr @"X#", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %6)
          to label %51 unwind label %landing

b13:
  %7 = load double, ptr @"X#", !tbaa !2
  %8 = fcmp ole double %7, 0.000000e+00
  br i1 %8, label %b14, label %b16

b14:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %53 unwind label %landing

b16:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %52 unwind label %landing

b17:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %b19 unwind label %landing

b19:
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %b20 unwind label %landing

b20:
  %9 = load i16, ptr @"K%", !tbaa !2
  %10 = load i16, ptr @$data.1, !tbaa !2
  %11 = add nsw i16 %10, %9
  store i16 %11, ptr @"K%", !tbaa !2
  br label %b7

b21:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 7, ptr @$data.2, !tbaa !2
  store i16 1, ptr @$data.3, !tbaa !2
  br label %b22

b22:
  %12 = phi i16 [ %30, %b41 ], [ 1, %b21 ]
  %13 = phi i16 [ %29, %b41 ], [ 1, %b21 ]
  %14 = icmp sgt i16 %13, -1
  %15 = load i16, ptr @$data.2, !tbaa !2
  br i1 %14, label %b23, label %b24

b23:
  %16 = icmp sgt i16 %12, %15
  br i1 %16, label %b26, label %b25

b24:
  %17 = icmp slt i16 %12, %15
  br i1 %17, label %b26, label %b25

b25:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
          to label %b27 unwind label %landing

b26:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b27:
  %18 = load double, ptr @"X#", !tbaa !2
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %18)
          to label %54 unwind label %landing

b28:
  %19 = load double, ptr @"X#", !tbaa !2
  %20 = call double @llvm.fabs.f64(double %19)
  %21 = fcmp ogt double %20, 0x41E0000000000000
  br i1 %21, label %b29, label %b31

b29:
  store i16 10, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %59 unwind label %landing

b31:
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
          to label %55 unwind label %landing

b32:
  %22 = load double, ptr @"X#", !tbaa !2
  %23 = call double @llvm.fabs.f64(double %22)
  %24 = fcmp ogt double %23, 0x41E0000000000000
  br i1 %24, label %b33, label %b35

b33:
  store i16 12, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %58 unwind label %landing

b35:
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %56 unwind label %landing

b36:
  %25 = load double, ptr @"X#", !tbaa !2
  %26 = call double @llvm.fabs.f64(double %25)
  %27 = fcmp ogt double %26, 0x41E0000000000000
  br i1 %27, label %b37, label %b39

b37:
  store i16 14, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %57 unwind label %landing

b39:
  store i16 15, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string11)
          to label %b40 unwind label %landing

b40:
  store i16 16, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %b41 unwind label %landing

b41:
  %28 = load i16, ptr @"K%", !tbaa !2
  %29 = load i16, ptr @$data.3, !tbaa !2
  %30 = add nsw i16 %29, %28
  store i16 %30, ptr @"K%", !tbaa !2
  br label %b22

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %31 = extractvalue { ptr, i32 } %landed, 1
  %32 = trunc i32 %31 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %32)
  %33 = load i16, ptr %site
  switch table i16 %33, label %34 [
    i16 0, label %34
    i16 1, label %35
    i16 2, label %36
    i16 3, label %37
    i16 4, label %38
    i16 5, label %39
    i16 6, label %40
    i16 7, label %41
    i16 8, label %42
    i16 9, label %43
    i16 10, label %44
    i16 11, label %45
    i16 12, label %46
    i16 13, label %47
    i16 14, label %48
    i16 15, label %49
    i16 16, label %50
  ]

34:
  br label %b6

35:
  br label %b12

36:
  br label %b13

37:
  br label %b16

38:
  br label %b17

39:
  br label %b19

40:
  br label %b20

41:
  br label %b21

42:
  br label %b27

43:
  br label %b28

44:
  br label %b31

45:
  br label %b32

46:
  br label %b35

47:
  br label %b36

48:
  br label %b39

49:
  br label %b40

50:
  br label %b41

51:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b13 unwind label %landing

52:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b17 unwind label %landing

53:
  unreachable

54:
  store i16 9, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b28 unwind label %landing

55:
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b32 unwind label %landing

56:
  store i16 13, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b36 unwind label %landing

57:
  unreachable

58:
  unreachable

59:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$RSTB(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$RDR8(ptr addrspace(1) nocapture noretain) addrspace(1)

declare cc 1005 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEOS() addrspace(1)

declare double @llvm.log.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sqrt.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare double @llvm.fabs.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.sin.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.cos.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
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
