target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"I%" = internal global [2 x i8] zeroinitializer
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"exp\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"log\00" }>
@$string10 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string10, i16 4), [4 x i8] c"sin\00" }>
@$string12 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string12, i16 4), [4 x i8] c"done" }>
@$string13 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string13, i16 4), [4 x i8] c"err\00" }>
@$data.2 = internal global i16 0
@$data.3 = internal global i16 0

define internal cc 1000 void @__main() addrspace(1) noinline "llrm-fp-eval"="x87-extended" noreturn personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  store i16 1, ptr @"I%", !tbaa !2
  %0 = fmul float 1.000000e+10, 1.000000e+03
  %1 = call float @llvm.fabs.f32(float %0)
  %2 = call float @llvm.sin.f32(float %0)
  %3 = fpround float %2 to float
  br label %b4

b4:
  %4 = phi i16 [ %10, %b12 ], [ 1, %frame ]
  %5 = icmp slt i16 %4, 4
  br i1 %5, label %b7, label %b14.thread

b7:
  %6 = sitofp i16 %4 to double
  %7 = fmul double %6, 3.000000e+04
  %8 = fcmp oge double %7, 0x40D62E2CD0B73AF5
  br i1 %8, label %b9, label %b11

b14.thread:
  store i16 0, ptr @"I%", !tbaa !2
  store i16 2, ptr @$data.2, !tbaa !2
  store i16 1, ptr @$data.3, !tbaa !2
  br label %b16

b9:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6)
          to label %33 unwind label %landing

b11:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
          to label %31 unwind label %landing

b12:
  %9 = load i16, ptr @"I%", !tbaa !2
  %10 = add nsw i16 %9, 1
  store i16 %10, ptr @"I%", !tbaa !2
  br label %b4

b13:
  %11 = load i16, ptr @"I%", !tbaa !2
  %12 = load i16, ptr @$data.3, !tbaa !2
  %13 = add nsw i16 %12, %11
  store i16 %13, ptr @"I%", !tbaa !2
  %14 = icmp sgt i16 %12, -1
  %15 = load i16, ptr @$data.2, !tbaa !2
  br i1 %14, label %b14, label %b15

b14:
  %16 = icmp sgt i16 %13, %15
  br i1 %16, label %b17, label %b16

b15:
  %17 = icmp slt i16 %13, %15
  br i1 %17, label %b17, label %b16

b16:
  %18 = phi i16 [ %13, %b14 ], [ %13, %b15 ], [ 0, %b14.thread ]
  %19 = add i16 %18, -1
  %20 = sitofp i16 %19 to float
  %21 = fcmp ole float %20, 0.000000e+00
  br i1 %21, label %b18, label %b20

b17:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string10)
          to label %37 unwind label %landing

b18:
  store i16 2, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %36 unwind label %landing

b20:
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
          to label %34 unwind label %landing

b23:
  store i16 4, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %39 unwind label %landing

b24:
  store i16 4, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PER4(float %3)
          to label %b25 unwind label %landing

b25:
  store i16 5, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string12)
          to label %b26 unwind label %landing

b26:
  call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %22 = extractvalue { ptr, i32 } %landed, 1
  %23 = trunc i32 %22 to i16
  %resumed = call fastcc addrspace(1) i16 @__main$handler(i16 %23)
  %24 = load i16, ptr %site
  switch table i16 %24, label %25 [
    i16 0, label %25
    i16 1, label %26
    i16 2, label %27
    i16 3, label %28
    i16 4, label %29
    i16 5, label %30
  ]

25:
  br label %b11

26:
  br label %b12

27:
  br label %b20

28:
  br label %b13

29:
  br label %b25

30:
  br label %b26

31:
  %32 = load i16, ptr @"I%", !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %32)
          to label %b12 unwind label %landing

33:
  unreachable

34:
  %35 = load i16, ptr @"I%", !tbaa !2
  store i16 3, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %35)
          to label %b13 unwind label %landing

36:
  unreachable

37:
  %38 = fcmp ogt float %1, 0x41E0000000000000
  br i1 %38, label %b23, label %b24

39:
  unreachable
}

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 void @llrm.qb.B$PEI4(i32) addrspace(1)

declare double @llvm.exp.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare float @llvm.log.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.fabs.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.sin.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1)

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

define internal fastcc i16 @__main$handler(i16 %0) addrspace(1) "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
entry:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string13)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %0)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PEI4(i32 0)
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
