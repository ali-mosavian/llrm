target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i16 @_f(i16 %0, i16 %1, i16 %2) addrspace(1) memory(none) nounwind {
b1:
  %3 = sub i16 0, %2
  %4 = icmp ule i16 %2, 0
  br i1 %4, label %b3, label %11

b3:
  %5 = phi i16 [ 0, %b1 ], [ %9, %12 ]
  %6 = add i16 %5, %0
  ret i16 %6

b4:
  %7 = phi i16 [ %9, %b4 ], [ 0, %11 ]
  %lsr.iv2 = phi i16 [ %lsr.iv.next, %b4 ], [ %0, %11 ]
  %lsr.iv11 = phi i16 [ %lsr.iv.next1, %b4 ], [ %3, %11 ]
  %8 = udiv i16 %lsr.iv2, %1
  %9 = add i16 %7, %8
  %lsr.iv.next = add i16 %lsr.iv2, 1
  %lsr.iv.next1 = add i16 %lsr.iv11, 1
  %10 = icmp ne i16 %lsr.iv.next1, 0
  br i1 %10, label %b4, label %12

11:
  br label %b4

12:
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
