declare cc 1003 i32 @_bench_matmul(i16) memory(none) nounwind norecurse

declare i32 @_main() memory(readwrite, argmem: none)

declare cc 1003 i32 @_report(i32)

define internal cc 1003 i32 @pipeline.body(i16 %0) memory(none) nounwind norecurse {
b1:
  %1 = alloca [128 x i8]
  %2 = alloca [128 x i8]
  %3 = alloca [256 x i8]
  br label %b4

b3:
  br label %b13

b4:
  %4 = phi i32 [ %10, %b6 ], [ 0, %b1 ]
  %5 = phi i16 [ %9, %b6 ], [ 0, %b1 ]
  %6 = mul nsw i32 %4, 3
  %7 = getelementptr inbounds [16 x i8], ptr %1, i32 %4
  %8 = getelementptr inbounds [16 x i8], ptr %2, i32 %4
  br label %b7

b6:
  %9 = add i16 %5, 1
  %10 = zext i16 %9 to i32
  %11 = icmp ult i16 %9, 8
  br i1 %11, label %b4, label %b3

b7:
  %12 = phi i32 [ %26, %b9 ], [ 0, %b4 ]
  %13 = phi i16 [ %25, %b9 ], [ 0, %b4 ]
  %14 = add nsw i32 %6, %12
  %15 = add nsw i32 %14, 1
  %16 = trunc i32 %15 to i16
  %17 = add i16 %16, %0
  %18 = getelementptr inbounds i16, ptr %7, i32 %12
  store i16 %17, ptr %18, !tbaa !9
  %19 = icmp eq i16 %5, %13
  br i1 %19, label %b10, label %b8

b8:
  %20 = add nsw i32 %4, %12
  %21 = urem i32 %20, 3
  br label %b9

b9:
  %22 = phi i32 [ 2, %b10 ], [ %21, %b8 ]
  %23 = trunc nsw i32 %22 to i16
  %24 = getelementptr inbounds i16, ptr %8, i32 %12
  store i16 %23, ptr %24, !tbaa !9
  %25 = add i16 %13, 1
  %26 = zext i16 %25 to i32
  %27 = icmp ult i16 %25, 8
  br i1 %27, label %b7, label %b6

b10:
  br label %b9

b12:
  br label %b22

b13:
  %28 = phi i32 [ %33, %b15 ], [ 0, %b3 ]
  %29 = phi i16 [ %32, %b15 ], [ 0, %b3 ]
  %30 = getelementptr inbounds [16 x i8], ptr %1, i32 %28
  %31 = getelementptr inbounds [32 x i8], ptr %3, i32 %28
  br label %b16

b15:
  %32 = add i16 %29, 1
  %33 = zext i16 %32 to i32
  %34 = icmp ult i16 %32, 8
  br i1 %34, label %b13, label %b12

b16:
  %35 = phi i32 [ %40, %b18 ], [ 0, %b13 ]
  %36 = phi i16 [ %39, %b18 ], [ 0, %b13 ]
  br label %b19

b18:
  %37 = phi i32 [ %53, %b19 ]
  %38 = getelementptr inbounds i32, ptr %31, i32 %35
  store i32 %37, ptr %38, !tbaa !11
  %39 = add i16 %36, 1
  %40 = zext i16 %39 to i32
  %41 = icmp ult i16 %39, 8
  br i1 %41, label %b16, label %b15

b19:
  %42 = phi i32 [ %55, %b19 ], [ 0, %b16 ]
  %43 = phi i32 [ %53, %b19 ], [ 0, %b16 ]
  %44 = phi i16 [ %54, %b19 ], [ 0, %b16 ]
  %45 = getelementptr inbounds i16, ptr %30, i32 %42
  %46 = load i16, ptr %45, !tbaa !9
  %47 = sext i16 %46 to i32
  %48 = getelementptr inbounds [16 x i8], ptr %2, i32 %42
  %49 = getelementptr inbounds i16, ptr %48, i32 %35
  %50 = load i16, ptr %49, !tbaa !9
  %51 = sext i16 %50 to i32
  %52 = mul nsw i32 %47, %51
  %53 = add nsw i32 %43, %52
  %54 = add i16 %44, 1
  %55 = zext i16 %54 to i32
  %56 = icmp ult i16 %54, 8
  br i1 %56, label %b19, label %b18

b21:
  %57 = phi i32 [ %63, %b24 ]
  ret i32 %57

b22:
  %58 = phi i32 [ %65, %b24 ], [ 0, %b12 ]
  %59 = phi i32 [ %63, %b24 ], [ 0, %b12 ]
  %60 = phi i16 [ %64, %b24 ], [ 0, %b12 ]
  %61 = getelementptr inbounds [32 x i8], ptr %3, i32 %58
  %62 = mul nsw i32 %58, 8
  br label %b25

b24:
  %63 = phi i32 [ %75, %b25 ]
  %64 = add i16 %60, 1
  %65 = zext i16 %64 to i32
  %66 = icmp ult i16 %64, 8
  br i1 %66, label %b22, label %b21

b25:
  %67 = phi i32 [ %77, %b25 ], [ 0, %b22 ]
  %68 = phi i32 [ %75, %b25 ], [ %59, %b22 ]
  %69 = phi i16 [ %76, %b25 ], [ 0, %b22 ]
  %70 = getelementptr inbounds i32, ptr %61, i32 %67
  %71 = load i32, ptr %70, !tbaa !11
  %72 = add nsw i32 %62, %67
  %73 = add nsw i32 %72, 1
  %74 = mul i32 %71, %73
  %75 = add i32 %68, %74
  %76 = add i16 %69, 1
  %77 = zext i16 %76 to i32
  %78 = icmp ult i16 %76, 8
  br i1 %78, label %b25, label %b24
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
