@_bench_x_switch_via = global ptr addrspace(1) @"_bench_x_switch@3"

declare cc 1005 range(i16 0, 6) i16 @"_step@3"(i16, i16) memory(none) willreturn nounwind norecurse

declare cc 1005 i32 @"_bench_x_switch@3"(i16) addrspace(1) memory(none) nounwind norecurse willreturn

declare i16 @_main() addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

define internal cc 1005 i32 @pipeline.body(i16 %0) memory(none) nounwind norecurse willreturn {
b1:
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %16, %10 ]
  %2 = phi i16 [ 0, %b1 ], [ %11, %10 ]
  %3 = phi i32 [ 0, %b1 ], [ %15, %10 ]
  %4 = icmp slt i16 %1, %0
  br i1 %4, label %6, label %b3

b3:
  %5 = phi i32 [ %3, %b2 ]
  ret i32 %5

6:
  switch i16 %2, label %default.unreachable1 [
    i16 0, label %10
    i16 1, label %7
    i16 2, label %8
    i16 3, label %10
    i16 4, label %9
  ]

7:
  br label %10

8:
  br label %10

9:
  br label %10

default.unreachable1:
  unreachable

10:
  %11 = phi i16 [ 2, %6 ], [ 0, %9 ], [ 3, %7 ], [ 4, %8 ], [ 2, %6 ]
  %12 = and i16 %1, 3
  %13 = mul nuw nsw i16 %11, %12
  %14 = zext nneg i16 %13 to i32
  %15 = add nsw i32 %3, %14
  %16 = add nuw nsw i16 %1, 1
  br label %b2
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
