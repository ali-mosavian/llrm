declare regparm3cc void @"_f@3"(ptr nocapture, i16) addrspace(1) memory(argmem: readwrite) willreturn norecurse

define internal regparm3cc void @pipeline.body(ptr nocapture %0, i16 %1) memory(argmem: readwrite) willreturn norecurse {
b1:
  %y2 = alloca ptr
  %y3 = alloca i16
  store ptr %0, ptr %y2, !tbaa !21
  store i16 %1, ptr %y3, !tbaa !9
  %2 = load i16, ptr %0, !tbaa !9
  %3 = add nsw i16 %2, %1
  store i16 %3, ptr %0, !tbaa !9
  ret void
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
