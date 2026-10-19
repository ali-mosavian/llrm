@_bench_x_switch_via = global ptr addrspace(1) @"_bench_x_switch@3"

declare cc 1005 range(i16 0, 6) i16 @"_step@3"(i16, i16) memory(none) willreturn nounwind norecurse

declare cc 1005 i32 @"_bench_x_switch@3"(i16) addrspace(1) memory(none) nounwind norecurse willreturn

declare i16 @_main() addrspace(1)

declare cc 1005 i16 @"_report@3"(i32) addrspace(1)

define internal cc 1005 i32 @pipeline.body(i16 %0) memory(none) nounwind norecurse willreturn {
b1:
  br label %b2

b2:
  %1 = phi i16 [ 0, %b1 ], [ %34, %29 ]
  %2 = phi i16 [ 0, %b1 ], [ poison, %29 ]
  %3 = phi i32 [ 0, %b1 ], [ %33, %29 ]
  %4 = icmp slt i16 %1, %0
  br i1 %4, label %18, label %b3

b2.jt4:
  %5 = phi i16 [ %41, %35 ]
  %6 = phi i16 [ %36, %35 ]
  %7 = phi i32 [ %40, %35 ]
  %8 = icmp slt i16 %5, %0
  br i1 %8, label %19, label %b3

b2.jt0:
  %9 = phi i16 [ %50, %42 ]
  %10 = phi i16 [ %45, %42 ]
  %11 = phi i32 [ %49, %42 ]
  %12 = icmp slt i16 %9, %0
  br i1 %12, label %20, label %b3

b2.jt2:
  %13 = phi i16 [ %59, %51 ]
  %14 = phi i16 [ %54, %51 ]
  %15 = phi i32 [ %58, %51 ]
  %16 = icmp slt i16 %13, %0
  br i1 %16, label %21, label %b3

b3:
  %17 = phi i32 [ %3, %b2 ], [ %15, %b2.jt2 ], [ %11, %b2.jt0 ], [ %7, %b2.jt4 ]
  ret i32 %17

18:
  switch i16 %2, label %default.unreachable1 [
    i16 0, label %51
    i16 1, label %22
    i16 2, label %23
    i16 3, label %51
    i16 4, label %26
  ]

19:
  br label %26

20:
  br label %51

21:
  br label %23

22:
  br label %42

23:
  %24 = phi i32 [ %15, %21 ], [ %3, %18 ]
  %25 = phi i16 [ %13, %21 ], [ %1, %18 ]
  br label %35

26:
  %27 = phi i32 [ %7, %19 ], [ %3, %18 ]
  %28 = phi i16 [ %5, %19 ], [ %1, %18 ]
  br label %42

default.unreachable1:
  unreachable

29:
  %30 = and i16 undef, 3
  %31 = mul nuw nsw i16 poison, %30
  %32 = zext nneg i16 %31 to i32
  %33 = add nsw i32 undef, %32
  %34 = add nuw nsw i16 undef, 1
  br label %b2

35:
  %36 = phi i16 [ 4, %23 ]
  %37 = and i16 %25, 3
  %38 = mul nuw nsw i16 %36, %37
  %39 = zext nneg i16 %38 to i32
  %40 = add nsw i32 %24, %39
  %41 = add nuw nsw i16 %25, 1
  br label %b2.jt4

42:
  %43 = phi i32 [ %3, %22 ], [ %27, %26 ]
  %44 = phi i16 [ %1, %22 ], [ %28, %26 ]
  %45 = phi i16 [ 0, %26 ], [ 3, %22 ]
  %46 = and i16 %44, 3
  %47 = mul nuw nsw i16 %45, %46
  %48 = zext nneg i16 %47 to i32
  %49 = add nsw i32 %43, %48
  %50 = add nuw nsw i16 %44, 1
  br label %b2.jt0

51:
  %52 = phi i32 [ %11, %20 ], [ %3, %18 ], [ %3, %18 ]
  %53 = phi i16 [ %9, %20 ], [ %1, %18 ], [ %1, %18 ]
  %54 = phi i16 [ 2, %18 ], [ 2, %18 ], [ 2, %20 ]
  %55 = and i16 %53, 3
  %56 = mul nuw nsw i16 %54, %55
  %57 = zext nneg i16 %56 to i32
  %58 = add nsw i32 %52, %57
  %59 = add nuw nsw i16 %53, 1
  br label %b2.jt2
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
