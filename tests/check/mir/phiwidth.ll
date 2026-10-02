target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i32 @_f(ptr nocapture readonly %0, ptr nocapture readonly %1, ptr nocapture readonly %2, i16 %3, i16 %4, i16 %5) addrspace(1) memory(argmem: read) {
b1:
  %6 = shl i16 %5, 1
  %7 = icmp sle i16 %3, 0
  br label %b4

b3:
  %8 = phi i32 [ %21, %b6 ]
  %9 = phi i32 [ %22, %b6 ]
  %10 = phi i32 [ %26, %b6 ]
  %11 = phi i32 [ %23, %b6 ]
  %12 = add nsw i32 %8, 1
  %13 = add nsw i32 %12, %9
  %14 = add nsw i32 %13, %10
  %15 = add nsw i32 %14, %11
  ret i32 %15

b4:
  %16 = phi i32 [ 0, %b1 ], [ %21, %b6 ]
  %17 = phi i32 [ 0, %b1 ], [ %22, %b6 ]
  %18 = phi i32 [ 1, %b1 ], [ %26, %b6 ]
  %19 = phi i32 [ 2, %b1 ], [ %23, %b6 ]
  %lsr.iv21 = phi i16 [ 0, %b1 ], [ %lsr.iv.next1, %b6 ]
  %lsr.iv31 = phi i16 [ -3, %b1 ], [ %lsr.iv.next2, %b6 ]
  br i1 %7, label %b6, label %50

b6:
  %20 = phi i32 [ 0, %b4 ], [ %lsr.iv.next, %54 ]
  %21 = phi i32 [ %16, %b4 ], [ %45, %54 ]
  %22 = phi i32 [ %17, %b4 ], [ %47, %54 ]
  %23 = add i32 %20, %19
  %24 = lshr i32 %20, 1
  %25 = mul i32 %24, 3
  %26 = add i32 %25, %18
  %lsr.iv.next1 = add i16 %lsr.iv21, %6
  %lsr.iv.next2 = add i16 %lsr.iv31, 1
  %27 = icmp ne i16 %lsr.iv.next2, 0
  br i1 %27, label %b4, label %b3

b7:
  %28 = phi i32 [ %45, %b7 ], [ %16, %50 ]
  %29 = phi i32 [ %47, %b7 ], [ %17, %50 ]
  %30 = phi i16 [ %48, %b7 ], [ 0, %50 ]
  %lsr.iv1 = phi i32 [ %lsr.iv.next, %b7 ], [ 0, %50 ]
  %31 = getelementptr i8, ptr %51, i32 %lsr.iv1
  %32 = getelementptr i8, ptr %31, i16 16
  %33 = load i16, ptr %32, !tbaa !9
  %34 = sext i16 %33 to i32
  %35 = add nsw i32 %28, %34
  %36 = getelementptr i8, ptr %52, i32 %lsr.iv1
  %37 = getelementptr i8, ptr %36, i16 16
  %38 = load i16, ptr %37, !tbaa !9
  %39 = sext i16 %38 to i32
  %40 = add nsw i32 %35, %39
  %41 = getelementptr i8, ptr %53, i32 %lsr.iv1
  %42 = getelementptr i8, ptr %41, i16 16
  %43 = load i16, ptr %42, !tbaa !9
  %44 = sext i16 %43 to i32
  %45 = add nsw i32 %40, %44
  %46 = sext i16 %30 to i32
  %47 = add nsw i32 %29, %46
  %48 = add nsw i16 %30, 1
  %lsr.iv.next = add i32 %lsr.iv1, 2
  %49 = icmp ne i16 %48, %3
  br i1 %49, label %b7, label %54

50:
  %51 = getelementptr i8, ptr %0, i16 %lsr.iv21
  %52 = getelementptr i8, ptr %1, i16 %lsr.iv21
  %53 = getelementptr i8, ptr %2, i16 %lsr.iv21
  br label %b7

54:
  br label %b6
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
