target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X#" = internal global [8 x i8] zeroinitializer
@"Y#" = internal global [8 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$string7 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string7, i16 4), [2 x i8] c"ok" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c".\00" }>
@$string9 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string9, i16 4), [6 x i8] c"error\00" }>
@$data.0 = internal global i16 0
@$data.1 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 0, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RSTB(i16 0)
          to label %b5 unwind label %landing

b5:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 10, ptr @$data.0, !tbaa !2
  store i16 1, ptr @$data.1, !tbaa !2
  br label %b6

b6:
  %0 = phi i16 [ %27, %b30 ], [ 1, %b5 ]
  %1 = phi i16 [ %26, %b30 ], [ 1, %b5 ]
  %2 = icmp sgt i16 %1, -1
  %3 = load i16, ptr @$data.0, !tbaa !2
  br i1 %2, label %b7, label %b8

b7:
  %4 = icmp sgt i16 %0, %3
  br i1 %4, label %b10, label %b9

b8:
  %5 = icmp slt i16 %0, %3
  br i1 %5, label %b10, label %b9

b9:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"X#" to ptr addrspace(1)))
          to label %37 unwind label %landing

b10:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b11:
  %6 = load double, ptr @"X#", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %6)
          to label %38 unwind label %landing

b12:
  %7 = load double, ptr @"X#", !tbaa !2
  %8 = load double, ptr @"Y#", !tbaa !2
  %9 = fcmp ogt double %7, 0.000000e+00
  br i1 %9, label %b13, label %b14

b13:
  %10 = call double @llvm.log2.f64(double %7)
  %11 = fmul double %10, %8
  %12 = fcmp oge double %11, 3.276750e+04
  br i1 %12, label %b22, label %b28

b14:
  %13 = fcmp oeq double %7, 0.000000e+00
  br i1 %13, label %b15, label %b17

b15:
  %14 = fcmp ogt double %8, 0.000000e+00
  br i1 %14, label %b28, label %b16

b16:
  %15 = fcmp oeq double %8, 0.000000e+00
  br i1 %15, label %b28, label %b20

b17:
  %16 = call i32 @llvm.lrint.i32.f64(double %8)
  %17 = sitofp i32 %16 to double
  %18 = fcmp oeq double %17, %8
  br i1 %18, label %b18, label %b20

b18:
  %19 = fneg double %7
  %20 = call double @llvm.log2.f64(double %19)
  %21 = fmul double %20, %8
  %22 = fcmp oge double %21, 3.276750e+04
  br i1 %22, label %b26, label %b27

b20:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %42 unwind label %landing

b22:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %43 unwind label %landing

b26:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %41 unwind label %landing

b27:
  %23 = and i32 %16, 1
  %24 = icmp eq i32 %23, 0
  br i1 %24, label %b28, label %b28

b28:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string7)
          to label %b29 unwind label %landing

b29:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
          to label %b30 unwind label %landing

b30:
  %25 = load i16, ptr @"K%", !tbaa !2
  %26 = load i16, ptr @$data.1, !tbaa !2
  %27 = add nsw i16 %26, %25
  store i16 %27, ptr @"K%", !tbaa !2
  br label %b6

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %28 = extractvalue { ptr, i32 } %landed, 1
  %29 = trunc i32 %28 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %29)
  %30 = load i16, ptr %site
  switch table i16 %30, label %31 [
    i16 0, label %31
    i16 1, label %32
    i16 2, label %33
    i16 3, label %34
    i16 4, label %35
    i16 5, label %36
  ]

31:
  br label %b5

32:
  br label %b11

33:
  br label %b12

34:
  br label %b28

35:
  br label %b29

36:
  br label %b30

37:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR8(ptr addrspace(1) addrspacecast (ptr @"Y#" to ptr addrspace(1)))
          to label %b11 unwind label %landing

38:
  %39 = load double, ptr @"Y#", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %39)
          to label %40 unwind label %landing

40:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b12 unwind label %landing

41:
  unreachable

42:
  unreachable

43:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$RSTB(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$RDR8(ptr addrspace(1) nocapture noretain) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSR8(double) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEOS() addrspace(1)

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string9)
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
