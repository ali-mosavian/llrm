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
  %1 = phi i32 [ 0, %b1 ], [ %17, %b9 ]
  %2 = phi i16 [ 0, %b1 ], [ %35, %b9 ]
  %3 = sext i16 %2 to i32
  %4 = icmp slt i32 %3, 100
  br i1 %4, label %b4, label %b3

b3:
  ret i32 %1

b4:
  br label %b5

b5:
  %5 = phi i16 [ 0, %b4 ], [ %29, %b7 ]
  %6 = sext i16 %5 to i32
  %7 = icmp slt i32 %6, 100
  br i1 %7, label %b7, label %b6

b6:
  %8 = sext i16 %2 to i32
  %9 = getelementptr inbounds i16, ptr %0, i32 %8
  %10 = load i16, ptr %9, !tbaa !9
  %11 = sext i16 %2 to i32
  %12 = getelementptr inbounds i16, ptr @_near_b, i32 %11
  %13 = load i16, ptr %12, !tbaa !9
  %14 = sext i16 %10 to i32
  %15 = sext i16 %13 to i32
  %16 = add nsw i32 %14, %15
  %17 = add nsw i32 %1, %16
  br label %b8

b7:
  %18 = sext i16 %5 to i32
  %19 = getelementptr inbounds i16, ptr @_near_a, i32 %18
  %20 = load i16, ptr %19, !tbaa !9
  %21 = sext i16 %20 to i32
  %22 = sext i16 %2 to i32
  %23 = add nsw i32 %21, %22
  %24 = trunc i32 %23 to i16
  %25 = sext i16 %5 to i32
  %26 = getelementptr inbounds i16, ptr @_near_b, i32 %25
  store i16 %24, ptr %26, !tbaa !9
  %27 = sext i16 %5 to i32
  %28 = add nsw i32 %27, 1
  %29 = trunc i32 %28 to i16
  br label %b5

b8:
  %30 = phi i16 [ 0, %b6 ], [ %43, %b10 ]
  %31 = sext i16 %30 to i32
  %32 = icmp slt i32 %31, 100
  br i1 %32, label %b10, label %b9

b9:
  %33 = sext i16 %2 to i32
  %34 = add nsw i32 %33, 1
  %35 = trunc i32 %34 to i16
  br label %b2

b10:
  %36 = sext i16 %30 to i32
  %37 = getelementptr inbounds i16, ptr @_near_b, i32 %36
  %38 = load i16, ptr %37, !tbaa !9
  %39 = sext i16 %30 to i32
  %40 = getelementptr inbounds i16, ptr @_near_a, i32 %39
  store i16 %38, ptr %40, !tbaa !9
  %41 = sext i16 %30 to i32
  %42 = add nsw i32 %41, 1
  %43 = trunc i32 %42 to i16
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
