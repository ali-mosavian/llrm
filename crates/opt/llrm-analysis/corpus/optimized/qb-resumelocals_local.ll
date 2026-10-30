target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$string3 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string3, i16 4), [4 x i8] c"next" }>
@$string4 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string4, i16 4), [4 x i8] c"err\00" }>
@$string5 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string5, i16 4), [6 x i8] c"again\00" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"err\00" }>
@$string7 = internal constant <{ [2 x i8], ptr, [6 x i8] }> <{ [2 x i8] c"\05\00", ptr getelementptr (i8, ptr @$string7, i16 4), [6 x i8] c"label\00" }>
@$string8 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\03\00", ptr getelementptr (i8, ptr @$string8, i16 4), [4 x i8] c"err\00" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn {
b1:
  tail call cc 1000 addrspace(1) void @NXT()
  tail call cc 1000 addrspace(1) void @AGN()
  tail call cc 1000 addrspace(1) void @LAB()
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 void @NXT() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  store i16 0, ptr %0
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  store i16 7, ptr %0, !tbaa !2
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5)
          to label %5 unwind label %landing

b5:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string3)
          to label %6 unwind label %landing

b6:
  ret void

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string4)
  %1 = extractvalue { ptr, i32 } %landed, 1
  %2 = trunc i32 %1 to i16
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %2)
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  %3 = load i16, ptr %site
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b5, label %b6

5:
  unreachable

6:
  %7 = load i16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
          to label %b6 unwind label %landing
}

define cc 1000 void @AGN() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %1)
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %2)
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  store i16 20, ptr %2, !tbaa !2
  br label %b4

b4:
  %3 = load i16, ptr %0, !tbaa !2
  %4 = icmp eq i16 %3, 0
  br i1 %4, label %b5, label %b6

b5:
  store i16 0, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 11)
          to label %10 unwind label %landing

b6:
  %5 = sdiv i16 10, %3
  store i16 %5, ptr %1, !tbaa !2
  br label %b7

b7:
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
          to label %11 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string6)
  %6 = extractvalue { ptr, i32 } %landed, 1
  %7 = trunc i32 %6 to i16
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %7)
  store i16 2, ptr %0, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  %8 = load i16, ptr %site
  %9 = icmp eq i16 %8, 0
  br i1 %9, label %b4, label %b7

10:
  unreachable

11:
  %12 = load i16, ptr %2, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSI2(i16 %12)
          to label %13 unwind label %landing

13:
  %14 = load i16, ptr %1, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %14)
          to label %15 unwind label %landing

15:
  ret void
}

define cc 1000 void @LAB() addrspace(1) noinline "llrm-fp-eval"="x87-extended" personality ptr addrspace(1) @llrm.qb.personality {
frame:
  %0 = alloca i16
  %site = alloca i16
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %0)
  store i16 0, ptr %0
  call addrspace(1) void @llrm.qb.frame(ptr nonnull %site)
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  store i16 0, ptr %site
  %1 = invoke cc 1005 addrspace(1) ptr @llrm.qb.B$RND0()
          to label %7 unwind label %landing

landing:
  %landed = landingpad { ptr, i32 }
          catch ptr null
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 false)
  call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string8)
  %2 = extractvalue { ptr, i32 } %landed, 1
  %3 = trunc i32 %2 to i16
  call cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %3)
  call cc 1000 addrspace(1) void @llrm.qb.onlocalerror(i1 true)
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string7)
          to label %4 unwind label %landing

4:
  %5 = load i16, ptr %0, !tbaa !2
  store i16 2, ptr %site
  invoke cc 1005 addrspace(1) void @llrm.qb.B$PEI2(i16 %5)
          to label %6 unwind label %landing

6:
  ret void

7:
  %8 = load float, ptr %1
  %9 = fmul float %8, 0.000000e+00
  %10 = call float @llvm.rint.f32(float %9)
  %11 = fcmp olt float %9, %10
  %12 = sext i1 %11 to i16
  %13 = sitofp i16 %12 to float
  %14 = fadd float %10, %13
  %15 = fadd float %14, 4.100000e+01
  %16 = call i16 @llvm.lrint.i16.f32(float %15)
  store i16 %16, ptr %0, !tbaa !2
  store i16 1, ptr %site
  invoke cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 7)
          to label %17 unwind label %landing

17:
  unreachable
}

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nounwind noreturn

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1)

declare cc 1000 i16 @llrm.qb.B$FERR() addrspace(1) nounwind

declare cc 1005 void @llrm.qb.B$PEI2(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1) noreturn

declare cc 1005 void @llrm.qb.B$PSI2(i16) addrspace(1)

declare cc 1005 ptr @llrm.qb.B$RND0() addrspace(1)

declare float @llvm.rint.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i16 @llvm.lrint.i16.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llrm.qb.personality(...) addrspace(1)

declare cc 1000 void @llrm.qb.onlocalerror(i1) addrspace(1) nounwind frame_bound

declare cc 1000 void @llrm.qb.frame(ptr) addrspace(1) nounwind

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
