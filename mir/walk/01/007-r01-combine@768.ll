@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

declare cc 1002 i32 @walk_(ptr noundef) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse

declare i32 @_main()

declare cc 1002 i32 @report_(i32)

define internal cc 1002 i32 @pipeline.body(ptr noundef %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse {
b1:
  br label %b4

b3:
  ret i32 %11

b4:
  %1 = phi i32 [ 0, %b1 ], [ %19, %b9 ]
  %2 = phi i32 [ 0, %b1 ], [ %11, %b9 ]
  %3 = trunc nuw nsw i32 %1 to i16
  br label %b7

b6:
  %4 = getelementptr inbounds nuw [2 x i8], ptr %0, i32 %1
  %5 = load i16, ptr %4, !tbaa !9
  %6 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %1
  %7 = load i16, ptr %6, !tbaa !9
  %8 = sext i16 %5 to i32
  %9 = sext i16 %7 to i32
  %10 = add i32 %2, %8
  %11 = add i32 %10, %9
  br label %b10

b7:
  %12 = phi i32 [ 0, %b4 ], [ %17, %b7 ]
  %13 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %12
  %14 = load i16, ptr %13, !tbaa !9
  %15 = add i16 %14, %3
  %16 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %12
  store i16 %15, ptr %16, !tbaa !9
  %17 = add nuw nsw i32 %12, 1
  %18 = icmp ult i32 %17, 100
  br i1 %18, label %b7, label %b6

b9:
  %19 = add nsw i32 %1, 1
  %20 = icmp ult i32 %19, 100
  br i1 %20, label %b4, label %b3

b10:
  %21 = phi i32 [ 0, %b6 ], [ %25, %b10 ]
  %22 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %21
  %23 = load i16, ptr %22, !tbaa !9
  %24 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %21
  store i16 %23, ptr %24, !tbaa !9
  %25 = add nuw nsw i32 %21, 1
  %26 = icmp ult i32 %25, 100
  br i1 %26, label %b10, label %b9
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
