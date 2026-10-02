target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i16 @_f(ptr nocapture %0, i16 %1, i16 %2, i16 %3, i16 %4, i16 %5, i16 %6, i16 %7) addrspace(1) memory(argmem: readwrite) {
b1:
  %8 = shl i16 %7, 1
  %9 = sub i16 0, %8
  %10 = getelementptr i8, ptr %0, i16 %8
  %11 = icmp sle i16 %7, 0
  br i1 %11, label %b3, label %34

b3:
  %12 = phi i16 [ 0, %b1 ], [ %28, %35 ]
  %13 = phi i16 [ 1, %b1 ], [ %30, %35 ]
  %14 = add nsw i16 %12, %13
  ret i16 %14

b4:
  %15 = phi i16 [ %28, %b4 ], [ 0, %34 ]
  %16 = phi i16 [ %30, %b4 ], [ 1, %34 ]
  %lsr.iv1 = phi i16 [ %lsr.iv.next, %b4 ], [ %9, %34 ]
  %17 = getelementptr i8, ptr %10, i16 %lsr.iv1
  %18 = load i16, ptr %17, !tbaa !9
  %19 = mul nsw i16 %1, %18
  %20 = add nsw i16 %15, %19
  %21 = add nsw i16 %2, %20
  %22 = xor i16 %16, %21
  %23 = xor i16 %3, %22
  %24 = add nsw i16 %20, %23
  %25 = mul nsw i16 %4, %24
  %26 = add nsw i16 %22, %25
  %27 = add nsw i16 %5, %26
  %28 = xor i16 %24, %27
  %29 = xor i16 %6, %28
  %30 = add nsw i16 %26, %29
  %31 = add nsw i16 %28, %30
  %32 = getelementptr i8, ptr %10, i16 %lsr.iv1
  store i16 %31, ptr %32, !tbaa !9
  %lsr.iv.next = add i16 %lsr.iv1, 2
  %33 = icmp ne i16 %lsr.iv.next, 0
  br i1 %33, label %b4, label %35

34:
  br label %b4

35:
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
