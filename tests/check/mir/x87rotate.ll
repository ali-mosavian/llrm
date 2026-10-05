; RUN: llrm-mir %s
; CHECK: define {{.*}} @_rot(

; Ten floats rotating through a loop: a cycle of ten phi copies (llrm-c -O2, 11-gepoffset.ll).
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

define i32 @_rot(i16 %0, float %1) addrspace(1) memory(none) willreturn nounwind norecurse {
b1:
  %2 = fadd float %1, 1.000000e+00
  %3 = fadd float %1, 2.000000e+00
  %4 = fadd float %1, 3.000000e+00
  %5 = fadd float %1, 4.000000e+00
  %6 = fadd float %1, 5.000000e+00
  %7 = fadd float %1, 6.000000e+00
  %8 = fadd float %1, 7.000000e+00
  %9 = fadd float %1, 8.000000e+00
  %10 = fadd float %1, 9.000000e+00
  %11 = sub i16 0, %0
  %12 = icmp sle i16 %0, 0
  br i1 %12, label %b3, label %55

b3:
  %13 = phi float [ %1, %b1 ], [ %53, %56 ]
  %14 = phi float [ %2, %b1 ], [ %44, %56 ]
  %15 = phi float [ %3, %b1 ], [ %45, %56 ]
  %16 = phi float [ %4, %b1 ], [ %46, %56 ]
  %17 = phi float [ %5, %b1 ], [ %47, %56 ]
  %18 = phi float [ %6, %b1 ], [ %48, %56 ]
  %19 = phi float [ %7, %b1 ], [ %49, %56 ]
  %20 = phi float [ %8, %b1 ], [ %50, %56 ]
  %21 = phi float [ %9, %b1 ], [ %51, %56 ]
  %22 = phi float [ %10, %b1 ], [ %42, %56 ]
  %23 = fmul float %14, 2.000000e+00
  %24 = fadd float %13, %23
  %25 = fmul float %15, 3.000000e+00
  %26 = fadd float %24, %25
  %27 = fmul float %16, 4.000000e+00
  %28 = fadd float %26, %27
  %29 = fmul float %17, 5.000000e+00
  %30 = fadd float %28, %29
  %31 = fmul float %18, 6.000000e+00
  %32 = fadd float %30, %31
  %33 = fmul float %19, 7.000000e+00
  %34 = fadd float %32, %33
  %35 = fmul float %20, 8.000000e+00
  %36 = fadd float %34, %35
  %37 = fmul float %21, 9.000000e+00
  %38 = fadd float %36, %37
  %39 = fmul float %22, 1.000000e+01
  %40 = fadd float %38, %39
  %41 = fptosi float %40 to i32
  ret i32 %41

b4:
  %42 = phi float [ %53, %b4 ], [ %1, %55 ]
  %43 = phi float [ %44, %b4 ], [ %2, %55 ]
  %44 = phi float [ %45, %b4 ], [ %3, %55 ]
  %45 = phi float [ %46, %b4 ], [ %4, %55 ]
  %46 = phi float [ %47, %b4 ], [ %5, %55 ]
  %47 = phi float [ %48, %b4 ], [ %6, %55 ]
  %48 = phi float [ %49, %b4 ], [ %7, %55 ]
  %49 = phi float [ %50, %b4 ], [ %8, %55 ]
  %50 = phi float [ %51, %b4 ], [ %9, %55 ]
  %51 = phi float [ %42, %b4 ], [ %10, %55 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ %11, %55 ]
  %52 = fmul float %42, 5.000000e-01
  %53 = fadd float %43, %52
  %lsr.iv.next = add i16 %lsr.iv1, 1
  %54 = icmp ne i16 %lsr.iv.next, 0
  br i1 %54, label %b4, label %56

55:
  br label %b4

56:
  br label %b3
}

define i16 @_main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  %0 = call addrspace(1) i32 @_rot(i16 7, float 1.000000e+00)
  %1 = call addrspace(1) i16 @_report(i32 %0)
  ret i16 0
}

declare i16 @_report(i32) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!"omnipotent char", !5, i64 0}
!7 = !{!6, !6, i64 0}
!8 = !{!"int2", !6, i64 0}
!9 = !{!8, !8, i64 0}
!10 = !{!"int4", !6, i64 0}
!11 = !{!10, !10, i64 0}
!12 = !{!"int8", !6, i64 0}
!13 = !{!12, !12, i64 0}
!14 = !{!"float4", !6, i64 0}
!15 = !{!14, !14, i64 0}
!16 = !{!"float8", !6, i64 0}
!17 = !{!16, !16, i64 0}
!18 = !{!"float10", !6, i64 0}
!19 = !{!18, !18, i64 0}
!20 = !{!"pointer2", !6, i64 0}
!21 = !{!20, !20, i64 0}
!22 = !{!"pointer4", !6, i64 0}
!23 = !{!22, !22, i64 0}
