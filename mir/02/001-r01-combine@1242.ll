@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

declare cc 1002 i32 @walk_(ptr noundef) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse

declare i32 @_main()

declare cc 1002 i32 @report_(i32)

define internal i32 @pipeline.body() {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %8, %b4 ]
  %1 = sext i16 %0 to i32
  %2 = icmp slt i32 %1, 100
  br i1 %2, label %b4, label %b3

b3:
  %3 = call cc 1002 i32 @walk_(ptr noundef @_far_a)
  %4 = call cc 1002 i32 @report_(i32 noundef %3)
  ret i32 0

b4:
  %5 = getelementptr inbounds [2 x i8], ptr @_near_a, i32 %1
  store i16 %0, ptr %5, !tbaa !9
  %6 = mul i16 %0, 3
  %7 = getelementptr inbounds [2 x i8], ptr @_far_a, i32 %1
  store i16 %6, ptr %7, !tbaa !9
  %8 = add i16 %0, 1
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
