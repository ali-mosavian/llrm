; A loop walking ten arrays by pointer (llrm-c -O2 --cpu 386, 06-rotate.ll), as the pipeline hands it to isel.
; Regalloc used to spill its own reload forever: issue #104.
target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a9 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a8 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a6 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a5 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a4 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a1 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a0 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a2 = global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a7 = addrspace(1) global [600 x i8] zeroinitializer
@_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a3 = addrspace(1) global [600 x i8] zeroinitializer

define i32 @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum(ptr nocapture readonly %0, ptr addrspace(1) nocapture readonly %1, ptr nocapture readonly %2, ptr addrspace(1) nocapture readonly %3, ptr nocapture readonly %4, i16 %5, i16 %6, i16 %7) addrspace(1) memory(readwrite, argmem: read) {
b1:
  %8 = alloca [600 x i8]
  %9 = alloca [600 x i8]
  %10 = addrspacecast ptr @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a2 to ptr addrspace(1)
  %11 = addrspacecast ptr %8 to ptr addrspace(1)
  %12 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %11, ptr addrspace(1) %10, i16 600)
  %13 = addrspacecast ptr @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a6 to ptr addrspace(1)
  %14 = addrspacecast ptr %9 to ptr addrspace(1)
  %15 = call addrspace(1) i16 @_lcopy(ptr addrspace(1) %14, ptr addrspace(1) %13, i16 600)
  %16 = add nsw i16 %5, 8
  %17 = mul nsw i16 %16, 2
  %18 = getelementptr inbounds i8, ptr %0, i16 %17
  %19 = getelementptr inbounds i8, ptr %0, i16 16
  %20 = getelementptr inbounds i8, ptr @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a1, i16 16
  %21 = getelementptr inbounds i8, ptr %8, i16 16
  %22 = getelementptr inbounds i8, ptr addrspace(1) %1, i16 16
  %23 = getelementptr inbounds i8, ptr %2, i16 16
  %24 = getelementptr inbounds i8, ptr @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a5, i16 16
  %25 = getelementptr inbounds i8, ptr %9, i16 16
  %26 = getelementptr inbounds i8, ptr addrspace(1) %3, i16 16
  %27 = getelementptr inbounds i8, ptr %4, i16 16
  %28 = getelementptr inbounds i8, ptr @_f_conc10_s2_xi_bgnlnpfpn_end_n_st1_sum_a9, i16 16
  br label %b2

b2:
  %29 = phi ptr [ %28, %b1 ], [ %82, %b4 ]
  %30 = phi ptr [ %27, %b1 ], [ %81, %b4 ]
  %31 = phi ptr addrspace(1) [ %26, %b1 ], [ %80, %b4 ]
  %32 = phi ptr [ %25, %b1 ], [ %79, %b4 ]
  %33 = phi ptr [ %24, %b1 ], [ %78, %b4 ]
  %34 = phi ptr [ %23, %b1 ], [ %77, %b4 ]
  %35 = phi ptr addrspace(1) [ %22, %b1 ], [ %76, %b4 ]
  %36 = phi ptr [ %21, %b1 ], [ %75, %b4 ]
  %37 = phi ptr [ %20, %b1 ], [ %74, %b4 ]
  %38 = phi ptr [ %19, %b1 ], [ %73, %b4 ]
  %39 = phi i32 [ 0, %b1 ], [ %72, %b4 ]
  %40 = icmp ult ptr %38, %18
  br i1 %40, label %b4, label %b3

b3:
  %41 = phi i32 [ %39, %b2 ]
  %42 = add nsw i32 %41, 1
  ret i32 %42

b4:
  %43 = load i16, ptr %38, !tbaa !9
  %44 = sext i16 %43 to i32
  %45 = add nsw i32 %39, %44
  %46 = load i16, ptr %37, !tbaa !9
  %47 = sext i16 %46 to i32
  %48 = add nsw i32 %45, %47
  %49 = load i16, ptr %36, !tbaa !9
  %50 = sext i16 %49 to i32
  %51 = add nsw i32 %48, %50
  %52 = load i16, ptr addrspace(1) %35, !tbaa !9
  %53 = sext i16 %52 to i32
  %54 = add nsw i32 %51, %53
  %55 = load i16, ptr %34, !tbaa !9
  %56 = sext i16 %55 to i32
  %57 = add nsw i32 %54, %56
  %58 = load i16, ptr %33, !tbaa !9
  %59 = sext i16 %58 to i32
  %60 = add nsw i32 %57, %59
  %61 = load i16, ptr %32, !tbaa !9
  %62 = sext i16 %61 to i32
  %63 = add nsw i32 %60, %62
  %64 = load i16, ptr addrspace(1) %31, !tbaa !9
  %65 = sext i16 %64 to i32
  %66 = add nsw i32 %63, %65
  %67 = load i16, ptr %30, !tbaa !9
  %68 = sext i16 %67 to i32
  %69 = add nsw i32 %66, %68
  %70 = load i16, ptr %29, !tbaa !9
  %71 = sext i16 %70 to i32
  %72 = add nsw i32 %69, %71
  %73 = getelementptr inbounds i8, ptr %38, i16 2
  %74 = getelementptr inbounds i8, ptr %37, i16 2
  %75 = getelementptr inbounds i8, ptr %36, i16 2
  %76 = getelementptr inbounds i8, ptr addrspace(1) %35, i16 2
  %77 = getelementptr inbounds i8, ptr %34, i16 2
  %78 = getelementptr inbounds i8, ptr %33, i16 2
  %79 = getelementptr inbounds i8, ptr %32, i16 2
  %80 = getelementptr inbounds i8, ptr addrspace(1) %31, i16 2
  %81 = getelementptr inbounds i8, ptr %30, i16 2
  %82 = getelementptr inbounds i8, ptr %29, i16 2
  br label %b2
}

declare i16 @_lcopy(ptr addrspace(1), ptr addrspace(1), i16) addrspace(1)

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
