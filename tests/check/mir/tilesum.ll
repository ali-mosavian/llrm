target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

@_tile = internal constant [8192 x i8] zeroinitializer

define i32 @_tile_sum(ptr nocapture readonly %0, ptr nocapture readonly %1, ptr nocapture readonly %2, ptr nocapture readonly %3) addrspace(1) memory(read, inaccessiblemem: none) {
b1:
  %4 = load i16, ptr %1, !tbaa !9
  br label %b2

b2:
  %5 = phi i16 [ 0, %b1 ], [ %14, %b6 ]
  %6 = phi i32 [ 0, %b1 ], [ %13, %b6 ]
  %7 = icmp slt i16 %5, %4
  br i1 %7, label %b4, label %b3

b3:
  %8 = phi i32 [ %6, %b2 ]
  ret i32 %8

b4:
  %9 = load i16, ptr %0, !tbaa !9
  br label %b5

b5:
  %10 = phi i16 [ 0, %b4 ], [ %28, %b7 ]
  %11 = phi i32 [ %6, %b4 ], [ %27, %b7 ]
  %12 = icmp slt i16 %10, %9
  br i1 %12, label %b7, label %b6

b6:
  %13 = phi i32 [ %11, %b5 ]
  %14 = add nsw i16 %5, 1
  br label %b2

b7:
  %15 = load i16, ptr %3, !tbaa !9
  %16 = add nsw i16 %5, %15
  %17 = and i16 %16, 63
  %18 = mul i16 %17, 128
  %19 = getelementptr inbounds i8, ptr @_tile, i16 %18
  %20 = load i16, ptr %2, !tbaa !9
  %21 = add nsw i16 %10, %20
  %22 = and i16 %21, 63
  %23 = mul i16 %22, 2
  %24 = getelementptr inbounds i8, ptr %19, i16 %23
  %25 = load i16, ptr %24, !tbaa !9
  %26 = sext i16 %25 to i32
  %27 = add nsw i32 %11, %26
  %28 = add nsw i16 %10, 1
  br label %b5
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
