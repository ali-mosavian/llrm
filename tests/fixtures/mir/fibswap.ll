target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i16 @_fib(i16 %0, i16 %1) addrspace(1) memory(none) nounwind {
b1:
  %2 = icmp ne i16 %1, 0
  br i1 %2, label %b3, label %b2

b2:
  %3 = sub nsw i16 %1, %0
  br label %b7

b3:
  %4 = sub i16 0, %0
  %5 = icmp sle i16 %0, 0
  br i1 %5, label %b5, label %15

b5:
  %6 = phi i16 [ 0, %b3 ], [ %9, %16 ]
  %7 = phi i16 [ 1, %b3 ], [ %10, %16 ]
  br label %b7

b6:
  %8 = phi i16 [ %9, %b6 ], [ 0, %15 ]
  %9 = phi i16 [ %10, %b6 ], [ 1, %15 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b6 ], [ %4, %15 ]
  %10 = add nsw i16 %8, %9
  %lsr.iv.next = add i16 %lsr.iv1, 1
  %11 = icmp ne i16 %lsr.iv.next, 0
  br i1 %11, label %b6, label %16

b7:
  %12 = phi i16 [ %0, %b2 ], [ %7, %b5 ]
  %13 = phi i16 [ %3, %b2 ], [ %6, %b5 ]
  %14 = sub nsw i16 %13, %12
  ret i16 %14

15:
  br label %b6

16:
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
