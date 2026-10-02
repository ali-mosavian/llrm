; RUN: llrm-mir %s
; CHECK: define {{.*}} @_horner(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16-n8:16:32"

define i32 @_horner(ptr nocapture readonly %0, i16 %1, i32 %2) addrspace(1) memory(argmem: read) {
b1:
  %3 = getelementptr i8, ptr %0, i16 0
  %4 = mul i16 %1, 2
  %5 = getelementptr i8, ptr %3, i16 %4
  %6 = sub i16 0, %4
  %7 = icmp sle i16 %1, 0
  br i1 %7, label %b3, label %19

b3:
  %8 = phi i32 [ 0, %b1 ], [ %21, %20 ]
  ret i32 %8

b4:
  %9 = phi i32 [ %16, %b4 ], [ 0, %19 ]
  %10 = phi i16 [ %17, %b4 ], [ %6, %19 ]
  %11 = mul nsw i32 %9, %2
  %12 = getelementptr i8, ptr %5, i16 %10
  %13 = load i16, ptr %12, !tbaa !9
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %11, %14
  %16 = srem i32 %15, 10007
  %17 = add i16 %10, 2
  %18 = icmp ne i16 %17, 0
  br i1 %18, label %b4, label %20

19:
  br label %b4

20:
  %21 = phi i32 [ %16, %b4 ]
  br label %b3
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
