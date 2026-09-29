; x87crowd.c's _deep as the pipeline hands it to isel (llrm-c -O2, 06-rotate.ll).
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

define i32 @_deep(ptr nocapture readonly %0, ptr nocapture readonly %1, ptr addrspace(1) nocapture readonly %2, float %3, float %4, float %5, float %6, i16 %7, ptr nocapture %8) addrspace(1) memory(read, argmem: readwrite, inaccessiblemem: none) {
b1:
  %9 = getelementptr inbounds i8, ptr %8, i16 0
  %10 = load float, ptr %9, !tbaa !15
  %11 = getelementptr inbounds i8, ptr %8, i16 4
  %12 = load float, ptr %11, !tbaa !15
  %13 = getelementptr inbounds i8, ptr %8, i16 8
  %14 = load float, ptr %13, !tbaa !15
  %15 = getelementptr inbounds i8, ptr %8, i16 12
  %16 = load float, ptr %15, !tbaa !15
  %17 = getelementptr inbounds i8, ptr %0, i16 0
  %18 = getelementptr inbounds i8, ptr %0, i16 4
  %19 = getelementptr inbounds i8, ptr %0, i16 8
  %20 = getelementptr inbounds i8, ptr %0, i16 12
  %21 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 0
  %22 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 0
  %23 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 4
  %24 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 8
  %25 = getelementptr inbounds i8, ptr addrspace(1) %21, i16 12
  %26 = getelementptr inbounds i8, ptr addrspace(1) %2, i16 16
  %27 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 0
  %28 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 4
  %29 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 8
  %30 = getelementptr inbounds i8, ptr addrspace(1) %26, i16 12
  br label %b2

b2:
  %31 = phi i16 [ 0, %b1 ], [ %64, %b5 ]
  %32 = phi float [ %12, %b1 ], [ %63, %b5 ]
  %33 = phi float [ %10, %b1 ], [ %61, %b5 ]
  %34 = phi i32 [ 0, %b1 ], [ %59, %b5 ]
  %35 = phi ptr [ %1, %b1 ], [ %65, %b5 ]
  %36 = icmp slt i16 %31, %7
  br i1 %36, label %b4, label %b3

b3:
  %37 = phi i32 [ %34, %b2 ]
  %38 = phi float [ %33, %b2 ]
  %39 = phi float [ %32, %b2 ]
  store float %38, ptr %9, !tbaa !15
  store float %39, ptr %11, !tbaa !15
  ret i32 %37

b4:
  %40 = load float, ptr %17, !tbaa !15
  %41 = getelementptr inbounds i8, ptr %35, i16 0
  %42 = load float, ptr %41, !tbaa !15
  %43 = fmul float %40, %42
  %44 = load float, ptr %18, !tbaa !15
  %45 = getelementptr inbounds i8, ptr %35, i16 4
  %46 = load float, ptr %45, !tbaa !15
  %47 = fmul float %44, %46
  %48 = fadd float %43, %47
  %49 = load float, ptr %19, !tbaa !15
  %50 = getelementptr inbounds i8, ptr %35, i16 8
  %51 = load float, ptr %50, !tbaa !15
  %52 = fmul float %49, %51
  %53 = fadd float %48, %52
  %54 = getelementptr inbounds i8, ptr %35, i16 12
  %55 = load float, ptr %54, !tbaa !15
  %56 = fsub float %53, %55
  %57 = load float, ptr %20, !tbaa !15
  %58 = fcmp olt float %56, %57
  br i1 %58, label %b6, label %b5

b5:
  %59 = phi i32 [ %34, %b4 ], [ %34, %b6 ], [ %34, %b7 ], [ %34, %b9 ], [ %34, %b10 ], [ %34, %b11 ], [ %105, %b12 ]
  %60 = fmul float %33, %14
  %61 = fadd float %60, %32
  %62 = fmul float %32, %16
  %63 = fadd float %62, %61
  %64 = add nsw i16 %31, 1
  %65 = getelementptr inbounds i8, ptr %35, i16 16
  br label %b2

b6:
  %66 = fneg float %57
  %67 = fcmp ogt float %56, %66
  br i1 %67, label %b7, label %b5

b7:
  %68 = fmul float %42, %56
  %69 = fsub float %40, %68
  %70 = fmul float %46, %56
  %71 = fsub float %44, %70
  %72 = fmul float %51, %56
  %73 = fsub float %49, %72
  %74 = fadd float %57, 1.000000e+00
  %75 = load float, ptr addrspace(1) %22, !tbaa !15
  %76 = fmul float %69, %75
  %77 = load float, ptr addrspace(1) %23, !tbaa !15
  %78 = fmul float %71, %77
  %79 = fadd float %76, %78
  %80 = load float, ptr addrspace(1) %24, !tbaa !15
  %81 = fmul float %73, %80
  %82 = fadd float %79, %81
  %83 = load float, ptr addrspace(1) %25, !tbaa !15
  %84 = fadd float %82, %83
  %85 = fsub float %84, %3
  %86 = load float, ptr addrspace(1) %27, !tbaa !15
  %87 = fmul float %69, %86
  %88 = load float, ptr addrspace(1) %28, !tbaa !15
  %89 = fmul float %71, %88
  %90 = fadd float %87, %89
  %91 = load float, ptr addrspace(1) %29, !tbaa !15
  %92 = fmul float %73, %91
  %93 = fadd float %90, %92
  %94 = load float, ptr addrspace(1) %30, !tbaa !15
  %95 = fadd float %93, %94
  %96 = fsub float %95, %4
  %97 = fneg float %74
  %98 = fcmp oge float %85, %97
  br i1 %98, label %b9, label %b5

b9:
  %99 = fadd float %5, %74
  %100 = fcmp ole float %85, %99
  br i1 %100, label %b10, label %b5

b10:
  %101 = fneg float %74
  %102 = fcmp oge float %96, %101
  br i1 %102, label %b11, label %b5

b11:
  %103 = fadd float %6, %74
  %104 = fcmp ole float %96, %103
  br i1 %104, label %b12, label %b5

b12:
  %105 = add nsw i32 %34, 1
  br label %b5
}

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
