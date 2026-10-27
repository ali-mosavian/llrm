@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

declare cc 1002 i32 @walk_(ptr noundef) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse

declare i32 @_main()

declare cc 1002 i32 @report_(i32)

define internal cc 1002 i32 @pipeline.body(ptr noundef %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse {
b1:
  br label %b2

b2:
  %1 = phi i32 [ 0, %b1 ], [ %13, %b9 ]
  %2 = phi i32 [ 0, %b1 ], [ %22, %b9 ]
  %3 = icmp ult i32 %2, 100
  br i1 %3, label %b4, label %b3

b3:
  ret i32 %1

b4:
  br label %b5

b5:
  %4 = phi i32 [ 0, %b4 ], [ %19, %b7 ]
  %5 = icmp ult i32 %4, 100
  br i1 %5, label %b7, label %b6

b6:
  %6 = getelementptr inbounds nuw [2 x i8], ptr %0, i32 %2
  %7 = load i16, ptr %6, !tbaa !9
  %8 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %2
  %9 = load i16, ptr %8, !tbaa !9
  %10 = sext i16 %7 to i32
  %11 = sext i16 %9 to i32
  %12 = add nsw i32 %10, %11
  %13 = add nsw i32 %1, %12
  br label %b8

b7:
  %14 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %4
  %15 = load i16, ptr %14, !tbaa !9
  %16 = trunc nuw nsw i32 %2 to i16
  %17 = add i16 %15, %16
  %18 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %4
  store i16 %17, ptr %18, !tbaa !9
  %19 = add nuw nsw i32 %4, 1
  br label %b5

b8:
  %20 = phi i32 [ 0, %b6 ], [ %26, %b10 ]
  %21 = icmp ult i32 %20, 100
  br i1 %21, label %b10, label %b9

b9:
  %22 = add nsw i32 %2, 1
  br label %b2

b10:
  %23 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %20
  %24 = load i16, ptr %23, !tbaa !9
  %25 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %20
  store i16 %24, ptr %25, !tbaa !9
  %26 = add nuw nsw i32 %20, 1
  br label %b8
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
