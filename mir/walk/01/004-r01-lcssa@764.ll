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
  %1 = phi i32 [ 0, %b1 ], [ %14, %b9 ]
  %2 = phi i32 [ 0, %b1 ], [ %23, %b9 ]
  %3 = icmp ult i32 %2, 100
  br i1 %3, label %b4, label %b3

b3:
  %4 = phi i32 [ %1, %b2 ]
  ret i32 %4

b4:
  br label %b5

b5:
  %5 = phi i32 [ 0, %b4 ], [ %20, %b7 ]
  %6 = icmp ult i32 %5, 100
  br i1 %6, label %b7, label %b6

b6:
  %7 = getelementptr inbounds nuw [2 x i8], ptr %0, i32 %2
  %8 = load i16, ptr %7, !tbaa !9
  %9 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %2
  %10 = load i16, ptr %9, !tbaa !9
  %11 = sext i16 %8 to i32
  %12 = sext i16 %10 to i32
  %13 = add i32 %11, %1
  %14 = add i32 %13, %12
  br label %b8

b7:
  %15 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %5
  %16 = load i16, ptr %15, !tbaa !9
  %17 = trunc nuw nsw i32 %2 to i16
  %18 = add i16 %16, %17
  %19 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %5
  store i16 %18, ptr %19, !tbaa !9
  %20 = add nuw nsw i32 %5, 1
  br label %b5

b8:
  %21 = phi i32 [ 0, %b6 ], [ %27, %b10 ]
  %22 = icmp ult i32 %21, 100
  br i1 %22, label %b10, label %b9

b9:
  %23 = add nsw i32 %2, 1
  br label %b2

b10:
  %24 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %21
  %25 = load i16, ptr %24, !tbaa !9
  %26 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %21
  store i16 %25, ptr %26, !tbaa !9
  %27 = add nuw nsw i32 %21, 1
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
