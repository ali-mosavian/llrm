target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

define internal cc 1002 i32 @walk_(ptr noundef %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse {
b1:
  br label %b2

b2:
  %1 = phi i32 [ 0, %b1 ], [ %15, %b9 ]
  %2 = phi i16 [ 0, %b1 ], [ %28, %b9 ]
  %3 = sext i16 %2 to i32
  %4 = icmp slt i32 %3, 100
  br i1 %4, label %b4, label %b3

b3:
  ret i32 %1

b4:
  br label %b5

b5:
  %5 = phi i16 [ 0, %b4 ], [ %23, %b7 ]
  %6 = sext i16 %5 to i32
  %7 = icmp slt i32 %6, 100
  br i1 %7, label %b7, label %b6

b6:
  %8 = getelementptr inbounds i16, ptr %0, i32 %3
  %9 = load i16, ptr %8, !tbaa !9
  %10 = getelementptr inbounds i16, ptr @_near_b, i32 %3
  %11 = load i16, ptr %10, !tbaa !9
  %12 = sext i16 %9 to i32
  %13 = sext i16 %11 to i32
  %14 = add nsw i32 %12, %13
  %15 = add nsw i32 %1, %14
  br label %b8

b7:
  %16 = getelementptr inbounds i16, ptr @_near_a, i32 %6
  %17 = load i16, ptr %16, !tbaa !9
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %18, %3
  %20 = trunc i32 %19 to i16
  %21 = getelementptr inbounds i16, ptr @_near_b, i32 %6
  store i16 %20, ptr %21, !tbaa !9
  %22 = add nsw i32 %6, 1
  %23 = trunc i32 %22 to i16
  br label %b5

b8:
  %24 = phi i16 [ 0, %b6 ], [ %33, %b10 ]
  %25 = sext i16 %24 to i32
  %26 = icmp slt i32 %25, 100
  br i1 %26, label %b10, label %b9

b9:
  %27 = add nsw i32 %3, 1
  %28 = trunc i32 %27 to i16
  br label %b2

b10:
  %29 = getelementptr inbounds i16, ptr @_near_b, i32 %25
  %30 = load i16, ptr %29, !tbaa !9
  %31 = getelementptr inbounds i16, ptr @_near_a, i32 %25
  store i16 %30, ptr %31, !tbaa !9
  %32 = add nsw i32 %25, 1
  %33 = trunc i32 %32 to i16
  br label %b8
}

define i32 @_main() {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %10, %b4 ]
  %1 = sext i16 %0 to i32
  %2 = icmp slt i32 %1, 100
  br i1 %2, label %b4, label %b3

b3:
  %3 = call cc 1002 i32 @walk_(ptr noundef @_far_a)
  %4 = call cc 1002 i32 @report_(i32 noundef %3)
  ret i32 0

b4:
  %5 = getelementptr inbounds i16, ptr @_near_a, i32 %1
  store i16 %0, ptr %5, !tbaa !9
  %6 = mul nsw i32 %1, 3
  %7 = trunc i32 %6 to i16
  %8 = getelementptr inbounds i16, ptr @_far_a, i32 %1
  store i16 %7, ptr %8, !tbaa !9
  %9 = add nsw i32 %1, 1
  %10 = trunc i32 %9 to i16
  br label %b2
}

declare cc 1002 i32 @report_(i32)

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
