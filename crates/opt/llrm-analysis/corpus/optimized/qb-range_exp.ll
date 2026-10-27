target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"X#" = internal global [8 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"K%" = internal global [2 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"ok" }>
@$string6 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string6, i16 4), [2 x i8] c".\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string8, i16 4), [2 x i8] c"ok" }>
@$string9 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\01\00", ptr getelementptr (i8, ptr @$string9, i16 4), [2 x i8] c".\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string10, i16 4), [6 x i8] c"error\00" }>
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
  store i16 8, ptr @$data.0, !tbaa !2
  store i16 1, ptr @$data.1, !tbaa !2
  br label %b7

b7:
  %0 = phi i16 [ %11, %b19 ], [ 1, %b6 ]
  %1 = phi i16 [ %10, %b19 ], [ 1, %b6 ]
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
  store i16 6, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RSTB(i16 1)
          to label %b20 unwind label %landing

b12:
  %6 = load double, ptr @"X#", !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR8(double %6)
          to label %39 unwind label %landing

b14:
  %7 = load double, ptr @"X#", !tbaa !2
  %8 = fcmp oge double %7, 0x40D62E2CD0B73AF5
  br i1 %8, label %b15, label %b17

b15:
  store i16 3, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %40 unwind label %landing

b17:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string5)
          to label %b18 unwind label %landing

b18:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
          to label %b19 unwind label %landing

b19:
  %9 = load i16, ptr @"K%", !tbaa !2
  %10 = load i16, ptr @$data.1, !tbaa !2
  %11 = add nsw i16 %10, %9
  store i16 %11, ptr @"K%", !tbaa !2
  br label %b7

b20:
  store i16 1, ptr @"K%", !tbaa !2
  store i16 4, ptr @$data.2, !tbaa !2
  store i16 1, ptr @$data.3, !tbaa !2
  br label %b21

b21:
  %12 = phi i16 [ %23, %b32 ], [ 1, %b20 ]
  %13 = phi i16 [ %22, %b32 ], [ 1, %b20 ]
  %14 = icmp sgt i16 %13, -1
  %15 = load i16, ptr @$data.2, !tbaa !2
  br i1 %14, label %b22, label %b23

b22:
  %16 = icmp sgt i16 %12, %15
  br i1 %16, label %b25, label %b24

b23:
  %17 = icmp slt i16 %12, %15
  br i1 %17, label %b25, label %b24

b24:
  store i16 7, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) addrspacecast (ptr @"S!" to ptr addrspace(1)))
          to label %b26 unwind label %landing

b25:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

b26:
  %18 = load float, ptr @"S!", !tbaa !2
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSR4(float %18)
          to label %41 unwind label %landing

b27:
  %19 = load float, ptr @"S!", !tbaa !2
  %20 = fcmp oge float %19, 0x40D62E2CE0000000
  br i1 %20, label %b28, label %b30

b28:
  store i16 9, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %42 unwind label %landing

b30:
  store i16 10, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string8)
          to label %b31 unwind label %landing

b31:
  store i16 11, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string9)
          to label %b32 unwind label %landing

b32:
  %21 = load i16, ptr @"K%", !tbaa !2
  %22 = load i16, ptr @$data.3, !tbaa !2
  %23 = add nsw i16 %22, %21
  store i16 %23, ptr @"K%", !tbaa !2
  br label %b21

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %24 = extractvalue { ptr, i32 } %landed, 1
  %25 = trunc i32 %24 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %25)
  %26 = load i16, ptr %site
  switch table i16 %26, label %27 [
    i16 0, label %27
    i16 1, label %28
    i16 2, label %29
    i16 3, label %30
    i16 4, label %31
    i16 5, label %32
    i16 6, label %33
    i16 7, label %34
    i16 8, label %35
    i16 9, label %36
    i16 10, label %37
    i16 11, label %38
  ]

27:
  br label %b6

28:
  br label %b12

29:
  br label %b14

30:
  br label %b17

31:
  br label %b18

32:
  br label %b19

33:
  br label %b20

34:
  br label %b26

35:
  br label %b27

36:
  br label %b30

37:
  br label %b31

38:
  br label %b32

39:
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b14 unwind label %landing

40:
  unreachable

41:
  store i16 8, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEOS()
          to label %b27 unwind label %landing

42:
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

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$RDR4(ptr addrspace(1) nocapture noretain) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSR4(float) addrspace(1)

declare float @llvm.exp.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
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
