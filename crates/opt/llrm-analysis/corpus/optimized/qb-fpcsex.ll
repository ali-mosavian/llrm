target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A!" = internal global [4 x i8] zeroinitializer
@"B!" = internal global [4 x i8] zeroinitializer
@"C!" = internal global [4 x i8] zeroinitializer
@$string5 = internal constant <{ [2 x i8], ptr, [2 x i8] }> <{ [2 x i8] c"\02\00", ptr getelementptr (i8, ptr @$string5, i16 4), [2 x i8] c"S=" }>
@$string6 = internal constant <{ [2 x i8], ptr, [4 x i8] }> <{ [2 x i8] c"\04\00", ptr getelementptr (i8, ptr @$string6, i16 4), [4 x i8] c"DONE" }>

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" norecurse {
b1:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) addrspacecast (ptr @"A!" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) addrspacecast (ptr @"B!" to ptr addrspace(1)))
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDR4(ptr addrspace(1) addrspacecast (ptr @"C!" to ptr addrspace(1)))
  %0 = load float, ptr @"A!", !tbaa !2
  %1 = load float, ptr @"B!", !tbaa !2
  %2 = fadd float %0, %1
  %3 = load float, ptr @"C!", !tbaa !2
  %4 = fmul float %2, %3
  %5 = fpround float %4 to float
  %6 = fdiv float %2, %3
  %7 = fpround float %6 to float
  %8 = fadd float %5, 0.000000e+00
  %9 = fadd float %8, %7
  %10 = fpround float %9 to float
  %11 = fadd float %10, %5
  %12 = fadd float %11, %7
  %13 = fpround float %12 to float
  %14 = fadd float %13, %5
  %15 = fadd float %14, %7
  %16 = fpround float %15 to float
  %17 = fadd float %16, %5
  %18 = fadd float %17, %7
  %19 = fpround float %18 to float
  %20 = fadd float %19, %5
  %21 = fadd float %20, %7
  %22 = fpround float %21 to float
  %23 = fadd float %22, %5
  %24 = fadd float %23, %7
  %25 = fpround float %24 to float
  %26 = fadd float %25, %5
  %27 = fadd float %26, %7
  %28 = fpround float %27 to float
  %29 = fadd float %28, %5
  %30 = fadd float %29, %7
  %31 = fpround float %30 to float
  %32 = fadd float %31, %5
  %33 = fadd float %32, %7
  %34 = fpround float %33 to float
  %35 = fadd float %34, %5
  %36 = fadd float %35, %7
  %37 = fpround float %36 to float
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PSSD(ptr @$string5)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PER4(float %37)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr @$string6)
  ret void
}

declare cc 1005 void @llrm.qb.B$RDR4(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PSSD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PER4(float) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
