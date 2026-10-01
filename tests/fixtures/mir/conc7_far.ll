target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a4 = addrspace(1) global [1800 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a5 = addrspace(1) global [3000 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a2 = addrspace(1) global [1200 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a3 = addrspace(1) global [2400 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a1 = addrspace(1) global [600 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a6 = addrspace(1) global [300 x i8] zeroinitializer
@_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1_a0 = addrspace(1) global [300 x i8] zeroinitializer

define i32 @_f_conc7_s1102468_xi_bpf_index_n_st1_sum_counteraffine_permute622_dup1(ptr addrspace(1) nocapture readonly %0, ptr addrspace(1) nocapture readonly %1, ptr addrspace(1) nocapture readonly %2, ptr addrspace(1) nocapture readonly %3, ptr addrspace(1) nocapture readonly %4, ptr addrspace(1) nocapture readonly %5, ptr addrspace(1) nocapture readonly %6, i16 %7, i16 %8, i16 %9) addrspace(1) memory(read, inaccessiblemem: none) {
b1:
  %10 = getelementptr i8, ptr addrspace(1) %4, i16 48
  %11 = getelementptr i8, ptr addrspace(1) %2, i16 32
  %12 = getelementptr i8, ptr addrspace(1) %1, i16 16
  %13 = getelementptr i8, ptr addrspace(1) %3, i16 64
  %14 = getelementptr i8, ptr addrspace(1) %5, i16 80
  %15 = add i16 8, %7
  %16 = sub i16 0, %7
  %17 = icmp sle i16 %7, 0
  br i1 %17, label %b3, label %51

b3:
  %18 = phi i32 [ 0, %b1 ], [ %55, %53 ]
  %19 = phi i32 [ 0, %b1 ], [ %54, %53 ]
  %20 = add nsw i32 %18, 1
  %21 = add nsw i32 %20, %19
  ret i32 %21

b4:
  %lsr.iv6 = phi i16 [ %lsr.iv.next5, %b4 ], [ %52, %51 ]
  %22 = phi i32 [ %48, %b4 ], [ 0, %51 ]
  %23 = phi i32 [ %25, %b4 ], [ 0, %51 ]
  %24 = phi i16 [ %49, %b4 ], [ %16, %51 ]
  %lsr.iv5 = phi ptr addrspace(1) [ %lsr.iv.next, %b4 ], [ %10, %51 ]
  %lsr.iv11 = phi ptr addrspace(1) [ %lsr.iv.next1, %b4 ], [ %11, %51 ]
  %lsr.iv21 = phi ptr addrspace(1) [ %lsr.iv.next2, %b4 ], [ %12, %51 ]
  %lsr.iv31 = phi ptr addrspace(1) [ %lsr.iv.next3, %b4 ], [ %13, %51 ]
  %lsr.iv41 = phi ptr addrspace(1) [ %lsr.iv.next4, %b4 ], [ %14, %51 ]
  %25 = add nsw i32 %23, 3
  %26 = load i16, ptr addrspace(1) %lsr.iv5, !tbaa !9
  %27 = sext i16 %26 to i32
  %28 = add nsw i32 %22, %27
  %29 = getelementptr inbounds i8, ptr addrspace(1) %6, i16 %lsr.iv6
  %30 = load i8, ptr addrspace(1) %29, !tbaa !7
  %31 = zext i8 %30 to i32
  %32 = add nsw i32 %28, %31
  %33 = load i32, ptr addrspace(1) %lsr.iv11, !tbaa !11
  %34 = add nsw i32 %32, %33
  %35 = load i16, ptr addrspace(1) %lsr.iv21, !tbaa !9
  %36 = sext i16 %35 to i32
  %37 = add nsw i32 %34, %36
  %38 = load double, ptr addrspace(1) %lsr.iv31, !tbaa !17
  %39 = fptosi double %38 to i32
  %40 = add nsw i32 %37, %39
  %41 = getelementptr inbounds i8, ptr addrspace(1) %0, i16 %lsr.iv6
  %42 = load i8, ptr addrspace(1) %41, !tbaa !7
  %43 = zext i8 %42 to i32
  %44 = add nsw i32 %40, %43
  %45 = load i16, ptr addrspace(1) %lsr.iv41, !tbaa !9
  %46 = sext i16 %45 to i32
  %47 = add nsw i32 %44, %46
  %48 = add nsw i32 %47, %36
  %49 = add i16 %24, 1
  %lsr.iv.next = getelementptr i8, ptr addrspace(1) %lsr.iv5, i16 6
  %lsr.iv.next1 = getelementptr i8, ptr addrspace(1) %lsr.iv11, i16 4
  %lsr.iv.next2 = getelementptr i8, ptr addrspace(1) %lsr.iv21, i16 2
  %lsr.iv.next3 = getelementptr i8, ptr addrspace(1) %lsr.iv31, i16 8
  %lsr.iv.next4 = getelementptr i8, ptr addrspace(1) %lsr.iv41, i16 10
  %50 = icmp ne i16 %49, 0
  %lsr.iv.next5 = add i16 %lsr.iv6, 1
  br i1 %50, label %b4, label %53

51:
  %52 = add i16 %16, %15
  br label %b4

53:
  %54 = phi i32 [ %25, %b4 ]
  %55 = phi i32 [ %48, %b4 ]
  br label %b3
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
