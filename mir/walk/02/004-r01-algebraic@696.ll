@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

declare cc 1002 i32 @walk_(ptr noundef nocapture readonly) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse willreturn

declare i32 @_main()

declare cc 1002 i32 @report_(i32)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

define internal i32 @pipeline.body() {
b1:
  br label %b2

b2:
  %0 = phi i16 [ 0, %b1 ], [ %30, %b4 ]
  %1 = sext i16 %0 to i32
  %2 = icmp slt i16 %0, 100
  br i1 %2, label %b4, label %3

3:
  br label %4

4:
  %5 = phi i32 [ 0, %3 ], [ %17, %8 ]
  %6 = phi i32 [ 0, %3 ], [ %16, %8 ]
  %7 = trunc nuw nsw i32 %5 to i16
  br label %18

8:
  %9 = getelementptr inbounds nuw [2 x i8], ptr @_far_a, i32 %5
  %10 = load i16, ptr %9, !tbaa !9
  %11 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %5
  %12 = load i16, ptr %11, !tbaa !9
  %13 = sext i16 %10 to i32
  %14 = sext i16 %12 to i32
  %15 = add i32 %13, %6
  %16 = add i32 %15, %14
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_near_a, ptr align 2 @_near_b, i32 200, i1 false)
  %17 = add nuw nsw i32 %5, 1
  %exitcond1 = icmp ne i32 %17, 100
  br i1 %exitcond1, label %4, label %25

18:
  %19 = phi i32 [ 0, %4 ], [ %24, %18 ]
  %20 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %19
  %21 = load i16, ptr %20, !tbaa !9
  %22 = add i16 %21, %7
  %23 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %19
  store i16 %22, ptr %23, !tbaa !9
  %24 = add nuw nsw i32 %19, 1
  %exitcond = icmp ne i32 %24, 100
  br i1 %exitcond, label %18, label %8

25:
  %26 = tail call cc 1002 i32 @report_(i32 noundef %16)
  ret i32 0

b4:
  %27 = getelementptr inbounds [2 x i8], ptr @_near_a, i32 %1
  store i16 %0, ptr %27, !tbaa !9
  %28 = mul i16 %0, 3
  %29 = getelementptr inbounds [2 x i8], ptr @_far_a, i32 %1
  store i16 %28, ptr %29, !tbaa !9
  %30 = add i16 %0, 1
  br label %b2
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
