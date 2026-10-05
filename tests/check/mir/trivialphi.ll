; RUN: llrm-mir %s
; CHECK: define {{.*}} @_bench_shellsort(

target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-i32:16-i64:16-n8:16:32"

define i32 @_bench_shellsort(i16 %0) addrspace(1) memory(none) nounwind {
b1:
  %1 = alloca [128 x i8]
  %2 = zext i16 %0 to i32
  br label %b4

b3:
  br label %b5

b4:
  %lsr.iv4 = phi i32 [ 37, %b1 ], [ %lsr.iv.next, %b4 ]
  %lsr.iv11 = phi i32 [ 0, %b1 ], [ %lsr.iv.next1, %b4 ]
  %3 = shl i32 %lsr.iv11, 6
  %4 = xor i32 %lsr.iv4, %3
  %5 = xor i32 %4, %2
  %6 = trunc i32 %5 to i16
  %7 = getelementptr i8, ptr %1, i32 %lsr.iv11
  store i16 %6, ptr %7, !tbaa !9
  %lsr.iv.next = add i32 %lsr.iv4, 109
  %lsr.iv.next1 = add i32 %lsr.iv11, 2
  %8 = icmp ne i32 %lsr.iv.next, 7013
  br i1 %8, label %b4, label %b3

b5:
  %9 = phi i16 [ 32, %b3 ], [ %13, %b9 ]
  %10 = icmp ne i16 %9, 0
  br i1 %10, label %b7, label %b6

b6:
  br label %b17

b7:
  br label %b8

b8:
  %11 = phi i16 [ %9, %b7 ], [ %22, %b12 ]
  %12 = icmp ult i16 %11, 64
  br i1 %12, label %b10, label %b9

b9:
  %13 = lshr i16 %9, 1
  br label %b5

b10:
  %14 = mul nuw i16 %11, 2
  %15 = getelementptr inbounds i8, ptr %1, i16 %14
  %16 = load i16, ptr %15, !tbaa !9
  br label %b11

b11:
  %17 = phi i16 [ %11, %b10 ], [ %23, %b14 ]
  %18 = icmp uge i16 %17, %9
  br i1 %18, label %b13, label %b12

b12:
  %19 = phi i16 [ %17, %b11 ], [ %17, %b13 ]
  %20 = mul nuw i16 %19, 2
  %21 = getelementptr inbounds i8, ptr %1, i16 %20
  store i16 %16, ptr %21, !tbaa !9
  %22 = add i16 %11, 1
  br label %b8

b13:
  %23 = sub i16 %17, %9
  %24 = mul nuw i16 %23, 2
  %25 = getelementptr inbounds i8, ptr %1, i16 %24
  %26 = load i16, ptr %25, !tbaa !9
  %27 = icmp ugt i16 %26, %16
  br i1 %27, label %b14, label %b12

b14:
  %28 = mul nuw i16 %17, 2
  %29 = getelementptr inbounds i8, ptr %1, i16 %28
  store i16 %26, ptr %29, !tbaa !9
  br label %b11

b16:
  %30 = phi i32 [ %37, %b17 ]
  ret i32 %30

b17:
  %31 = phi i32 [ 0, %b6 ], [ %37, %b17 ]
  %lsr.iv21 = phi i32 [ 0, %b6 ], [ %lsr.iv.next2, %b17 ]
  %lsr.iv31 = phi i16 [ -128, %b6 ], [ %lsr.iv.next3, %b17 ]
  %32 = getelementptr i8, ptr %1, i16 %lsr.iv31
  %33 = getelementptr i8, ptr %32, i16 128
  %34 = load i16, ptr %33, !tbaa !9
  %35 = zext i16 %34 to i32
  %lsr.iv.next2 = add i32 %lsr.iv21, 1
  %36 = mul i32 %35, %lsr.iv.next2
  %37 = add i32 %31, %36
  %lsr.iv.next3 = add i16 %lsr.iv31, 2
  %38 = icmp ne i16 %lsr.iv.next3, 0
  br i1 %38, label %b17, label %b16
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
