declare cc 1002 i32 @bench_shellsort_(i16) memory(none) nounwind norecurse

declare i32 @_main() memory(readwrite, argmem: none)

declare cc 1002 i32 @report_(i32)

define internal cc 1002 i32 @pipeline.body(i16 %0) memory(none) nounwind norecurse {
b1:
  %1 = alloca [128 x i8]
  br label %b2

b2:
  %widen.iv = phi i32 [ 0, %b1 ], [ %widen.iv.next, %b4 ]
  %2 = icmp ult i32 %widen.iv, 64
  br i1 %2, label %b4, label %b3

b3:
  br label %b5

b4:
  %3 = mul i32 %widen.iv, 109
  %4 = add i32 %3, 37
  %5 = shl i32 %widen.iv, 7
  %6 = xor i32 %4, %5
  %7 = trunc i32 %6 to i16
  %8 = xor i16 %7, %0
  %9 = mul nuw i32 %widen.iv, 2
  %10 = getelementptr inbounds i8, ptr %1, i32 %9
  store i16 %8, ptr %10, !tbaa !9
  %widen.iv.next = add i32 %widen.iv, 1
  br label %b2

b5:
  %11 = phi i16 [ 32, %b3 ], [ %17, %b9 ]
  %12 = zext i16 %11 to i32
  %13 = icmp ne i16 %11, 0
  br i1 %13, label %b7, label %b6

b6:
  br label %b15

b7:
  %14 = zext i16 %11 to i32
  br label %b8

b8:
  %widen.iv2 = phi i32 [ %14, %b7 ], [ %widen.iv.next2, %b12 ]
  %15 = icmp ult i32 %widen.iv2, 64
  br i1 %15, label %b10, label %b9

b9:
  %16 = ashr i32 %12, 1
  %17 = trunc i32 %16 to i16
  br label %b5

b10:
  %18 = mul nuw i32 %widen.iv2, 2
  %19 = getelementptr inbounds i8, ptr %1, i32 %18
  %20 = load i16, ptr %19, !tbaa !9
  %21 = trunc i32 %widen.iv2 to i16
  br label %b11

b11:
  %22 = phi i16 [ %21, %b10 ], [ %35, %b14 ]
  %23 = zext i16 %22 to i32
  %24 = icmp uge i16 %22, %11
  br i1 %24, label %b13, label %b12

b12:
  %25 = phi i32 [ %23, %b11 ], [ %23, %b13 ]
  %26 = mul nuw i32 %25, 2
  %27 = getelementptr inbounds i8, ptr %1, i32 %26
  store i16 %20, ptr %27, !tbaa !9
  %widen.iv.next2 = add i32 %widen.iv2, 1
  br label %b8

b13:
  %28 = sub nsw i32 %23, %12
  %29 = mul i32 %28, 2
  %30 = getelementptr inbounds i8, ptr %1, i32 %29
  %31 = load i16, ptr %30, !tbaa !9
  %32 = icmp ugt i16 %31, %20
  br i1 %32, label %b14, label %b12

b14:
  %33 = mul nuw i32 %23, 2
  %34 = getelementptr inbounds i8, ptr %1, i32 %33
  store i16 %31, ptr %34, !tbaa !9
  %35 = sub i16 %22, %11
  br label %b11

b15:
  %widen.iv1 = phi i32 [ 0, %b6 ], [ %widen.iv.next1, %b17 ]
  %36 = phi i32 [ 0, %b6 ], [ %45, %b17 ]
  %37 = icmp ult i32 %widen.iv1, 64
  br i1 %37, label %b17, label %b16

b16:
  %38 = phi i32 [ %36, %b15 ]
  ret i32 %38

b17:
  %39 = mul nuw i32 %widen.iv1, 2
  %40 = getelementptr inbounds i8, ptr %1, i32 %39
  %41 = load i16, ptr %40, !tbaa !9
  %42 = zext i16 %41 to i32
  %43 = add nsw i32 %widen.iv1, 1
  %44 = mul i32 %42, %43
  %45 = add i32 %36, %44
  %widen.iv.next1 = add i32 %widen.iv1, 1
  br label %b15
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
