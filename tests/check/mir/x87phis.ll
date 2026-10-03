; RUN: llrm-mir %s
; CHECK: define {{.*}} @_k(

; bench/fpbench-shaped n-body step (#358): nine floats copied into the loop phis at once (llrm-c -O2, 11-gepoffset.ll).
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-n8:16:32"

define i32 @_k(i16 %0) addrspace(1) memory(none) willreturn nounwind norecurse {
b1:
  %1 = alloca [24 x i8]
  %2 = alloca [24 x i8]
  %3 = alloca [24 x i8]
  %4 = alloca [24 x i8]
  %5 = getelementptr inbounds i8, ptr %1, i16 0
  store float 0.000000e+00, ptr %5, !tbaa !15
  %6 = getelementptr inbounds i8, ptr %2, i16 0
  store float 0.000000e+00, ptr %6, !tbaa !15
  %7 = getelementptr inbounds i8, ptr %3, i16 0
  store float 0.000000e+00, ptr %7, !tbaa !15
  %8 = getelementptr inbounds i8, ptr %4, i16 0
  store float 0.000000e+00, ptr %8, !tbaa !15
  %9 = getelementptr inbounds i8, ptr %1, i16 4
  store float 1.000000e+00, ptr %9, !tbaa !15
  %10 = getelementptr inbounds i8, ptr %2, i16 4
  store float 3.000000e+00, ptr %10, !tbaa !15
  %11 = getelementptr inbounds i8, ptr %3, i16 4
  store float 0.000000e+00, ptr %11, !tbaa !15
  %12 = getelementptr inbounds i8, ptr %4, i16 4
  store float 0.000000e+00, ptr %12, !tbaa !15
  %13 = getelementptr inbounds i8, ptr %1, i16 8
  store float 2.000000e+00, ptr %13, !tbaa !15
  %14 = getelementptr inbounds i8, ptr %2, i16 8
  store float 6.000000e+00, ptr %14, !tbaa !15
  %15 = getelementptr inbounds i8, ptr %3, i16 8
  store float 0.000000e+00, ptr %15, !tbaa !15
  %16 = getelementptr inbounds i8, ptr %4, i16 8
  store float 0.000000e+00, ptr %16, !tbaa !15
  %17 = getelementptr inbounds i8, ptr %1, i16 12
  store float 3.000000e+00, ptr %17, !tbaa !15
  %18 = getelementptr inbounds i8, ptr %2, i16 12
  store float 9.000000e+00, ptr %18, !tbaa !15
  %19 = getelementptr inbounds i8, ptr %3, i16 12
  store float 0.000000e+00, ptr %19, !tbaa !15
  %20 = getelementptr inbounds i8, ptr %4, i16 12
  store float 0.000000e+00, ptr %20, !tbaa !15
  %21 = getelementptr inbounds i8, ptr %1, i16 16
  store float 4.000000e+00, ptr %21, !tbaa !15
  %22 = getelementptr inbounds i8, ptr %2, i16 16
  store float 1.200000e+01, ptr %22, !tbaa !15
  %23 = getelementptr inbounds i8, ptr %3, i16 16
  store float 0.000000e+00, ptr %23, !tbaa !15
  %24 = getelementptr inbounds i8, ptr %4, i16 16
  store float 0.000000e+00, ptr %24, !tbaa !15
  %25 = getelementptr inbounds i8, ptr %1, i16 20
  store float 5.000000e+00, ptr %25, !tbaa !15
  %26 = getelementptr inbounds i8, ptr %2, i16 20
  store float 1.500000e+01, ptr %26, !tbaa !15
  %27 = getelementptr inbounds i8, ptr %3, i16 20
  store float 0.000000e+00, ptr %27, !tbaa !15
  %28 = getelementptr inbounds i8, ptr %4, i16 20
  store float 0.000000e+00, ptr %28, !tbaa !15
  %29 = sub i16 0, %0
  %30 = icmp sle i16 %0, 0
  br i1 %30, label %b6, label %156

b6:
  %31 = load float, ptr %7, !tbaa !15
  %32 = fmul float %31, 6.400000e+01
  %33 = fptosi float %32 to i32
  ret i32 %33

b7:
  %34 = phi float [ %47, %b9 ], [ 0.000000e+00, %156 ]
  %35 = phi float [ %49, %b9 ], [ 0.000000e+00, %156 ]
  %36 = phi float [ %51, %b9 ], [ 1.000000e+00, %156 ]
  %37 = phi float [ %53, %b9 ], [ 3.000000e+00, %156 ]
  %38 = phi float [ %55, %b9 ], [ 2.000000e+00, %156 ]
  %39 = phi float [ %57, %b9 ], [ 6.000000e+00, %156 ]
  %40 = phi float [ %59, %b9 ], [ 3.000000e+00, %156 ]
  %41 = phi float [ %61, %b9 ], [ 9.000000e+00, %156 ]
  %42 = phi float [ %63, %b9 ], [ 4.000000e+00, %156 ]
  %43 = phi float [ %65, %b9 ], [ 1.200000e+01, %156 ]
  %44 = phi float [ %67, %b9 ], [ 5.000000e+00, %156 ]
  %45 = phi float [ %69, %b9 ], [ 1.500000e+01, %156 ]
  %lsr.iv11 = phi i16 [ %lsr.iv.next1, %b9 ], [ %29, %156 ]
  br label %b10

b9:
  %46 = load float, ptr %7, !tbaa !15
  %47 = fadd float %34, %46
  store float %47, ptr %5, !tbaa !15
  %48 = load float, ptr %8, !tbaa !15
  %49 = fadd float %35, %48
  store float %49, ptr %6, !tbaa !15
  %50 = load float, ptr %11, !tbaa !15
  %51 = fadd float %36, %50
  store float %51, ptr %9, !tbaa !15
  %52 = load float, ptr %12, !tbaa !15
  %53 = fadd float %37, %52
  store float %53, ptr %10, !tbaa !15
  %54 = load float, ptr %15, !tbaa !15
  %55 = fadd float %38, %54
  store float %55, ptr %13, !tbaa !15
  %56 = load float, ptr %16, !tbaa !15
  %57 = fadd float %39, %56
  store float %57, ptr %14, !tbaa !15
  %58 = load float, ptr %19, !tbaa !15
  %59 = fadd float %40, %58
  store float %59, ptr %17, !tbaa !15
  %60 = load float, ptr %20, !tbaa !15
  %61 = fadd float %41, %60
  store float %61, ptr %18, !tbaa !15
  %62 = load float, ptr %23, !tbaa !15
  %63 = fadd float %42, %62
  store float %63, ptr %21, !tbaa !15
  %64 = load float, ptr %24, !tbaa !15
  %65 = fadd float %43, %64
  store float %65, ptr %22, !tbaa !15
  %66 = load float, ptr %27, !tbaa !15
  %67 = fadd float %44, %66
  store float %67, ptr %25, !tbaa !15
  %68 = load float, ptr %28, !tbaa !15
  %69 = fadd float %45, %68
  store float %69, ptr %26, !tbaa !15
  %lsr.iv.next1 = add i16 %lsr.iv11, 1
  %70 = icmp ne i16 %lsr.iv.next1, 0
  br i1 %70, label %b7, label %157

b10:
  %lsr.iv2 = phi i16 [ -24, %b7 ], [ %lsr.iv.next, %b10 ]
  %71 = getelementptr i8, ptr %1, i16 %lsr.iv2
  %72 = getelementptr i8, ptr %71, i16 24
  %73 = load float, ptr %72, !tbaa !15
  %74 = fsub float %34, %73
  %75 = getelementptr i8, ptr %2, i16 %lsr.iv2
  %76 = getelementptr i8, ptr %75, i16 24
  %77 = load float, ptr %76, !tbaa !15
  %78 = fsub float %35, %77
  %79 = fmul float %74, %74
  %80 = fmul float %78, %78
  %81 = fadd float %79, %80
  %82 = fadd float %81, 1.000000e+00
  %83 = fdiv float 1.000000e+00, %82
  %84 = fmul float %74, %83
  %85 = fadd float %84, 0.000000e+00
  %86 = fmul float %78, %83
  %87 = fadd float %86, 0.000000e+00
  %88 = fsub float %36, %73
  %89 = fsub float %37, %77
  %90 = fmul float %88, %88
  %91 = fmul float %89, %89
  %92 = fadd float %90, %91
  %93 = fadd float %92, 1.000000e+00
  %94 = fdiv float 1.000000e+00, %93
  %95 = fmul float %88, %94
  %96 = fadd float %85, %95
  %97 = fmul float %89, %94
  %98 = fadd float %87, %97
  %99 = fsub float %38, %73
  %100 = fsub float %39, %77
  %101 = fmul float %99, %99
  %102 = fmul float %100, %100
  %103 = fadd float %101, %102
  %104 = fadd float %103, 1.000000e+00
  %105 = fdiv float 1.000000e+00, %104
  %106 = fmul float %99, %105
  %107 = fadd float %96, %106
  %108 = fmul float %100, %105
  %109 = fadd float %98, %108
  %110 = fsub float %40, %73
  %111 = fsub float %41, %77
  %112 = fmul float %110, %110
  %113 = fmul float %111, %111
  %114 = fadd float %112, %113
  %115 = fadd float %114, 1.000000e+00
  %116 = fdiv float 1.000000e+00, %115
  %117 = fmul float %110, %116
  %118 = fadd float %107, %117
  %119 = fmul float %111, %116
  %120 = fadd float %109, %119
  %121 = fsub float %42, %73
  %122 = fsub float %43, %77
  %123 = fmul float %121, %121
  %124 = fmul float %122, %122
  %125 = fadd float %123, %124
  %126 = fadd float %125, 1.000000e+00
  %127 = fdiv float 1.000000e+00, %126
  %128 = fmul float %121, %127
  %129 = fadd float %118, %128
  %130 = fmul float %122, %127
  %131 = fadd float %120, %130
  %132 = fsub float %44, %73
  %133 = fsub float %45, %77
  %134 = fmul float %132, %132
  %135 = fmul float %133, %133
  %136 = fadd float %134, %135
  %137 = fadd float %136, 1.000000e+00
  %138 = fdiv float 1.000000e+00, %137
  %139 = fmul float %132, %138
  %140 = fadd float %129, %139
  %141 = fmul float %133, %138
  %142 = fadd float %131, %141
  %143 = getelementptr i8, ptr %3, i16 %lsr.iv2
  %144 = getelementptr i8, ptr %143, i16 24
  %145 = load float, ptr %144, !tbaa !15
  %146 = fadd float %145, %140
  %147 = getelementptr i8, ptr %3, i16 %lsr.iv2
  %148 = getelementptr i8, ptr %147, i16 24
  store float %146, ptr %148, !tbaa !15
  %149 = getelementptr i8, ptr %4, i16 %lsr.iv2
  %150 = getelementptr i8, ptr %149, i16 24
  %151 = load float, ptr %150, !tbaa !15
  %152 = fadd float %151, %142
  %153 = getelementptr i8, ptr %4, i16 %lsr.iv2
  %154 = getelementptr i8, ptr %153, i16 24
  store float %152, ptr %154, !tbaa !15
  %lsr.iv.next = add i16 %lsr.iv2, 4
  %155 = icmp ne i16 %lsr.iv.next, 0
  br i1 %155, label %b10, label %b9

156:
  br label %b7

157:
  br label %b6
}

define i16 @_main() addrspace(1) memory(readwrite, argmem: none) {
b1:
  %0 = call addrspace(1) i32 @_k(i16 20)
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
