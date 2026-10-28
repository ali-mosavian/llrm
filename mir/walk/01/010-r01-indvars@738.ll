@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

declare cc 1002 i32 @walk_(ptr noundef) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse

declare i32 @_main()

declare cc 1002 i32 @report_(i32)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

define internal cc 1002 i32 @pipeline.body(ptr noundef %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse {
b1:
  br label %b4

b3:
  %1 = phi i32 [ %12, %b9 ]
  ret i32 %1

b4:
  %2 = phi i32 [ 0, %b1 ], [ %19, %b9 ]
  %3 = phi i32 [ 0, %b1 ], [ %12, %b9 ]
  %4 = trunc nuw nsw i32 %2 to i16
  br label %b7

b6:
  %5 = getelementptr inbounds nuw [2 x i8], ptr %0, i32 %2
  %6 = load i16, ptr %5, !tbaa !9
  %7 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %2
  %8 = load i16, ptr %7, !tbaa !9
  %9 = sext i16 %6 to i32
  %10 = sext i16 %8 to i32
  %11 = add i32 %3, %9
  %12 = add i32 %11, %10
  call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_near_a, ptr align 2 @_near_b, i32 200, i1 false)
  br label %b10

b7:
  %13 = phi i32 [ 0, %b4 ], [ %18, %b7 ]
  %14 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %13
  %15 = load i16, ptr %14, !tbaa !9
  %16 = add i16 %15, %4
  %17 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %13
  store i16 %16, ptr %17, !tbaa !9
  %18 = add nuw nsw i32 %13, 1
  %exitcond = icmp ne i32 %18, 100
  br i1 %exitcond, label %b7, label %b6

b9:
  %19 = add nuw nsw i32 %2, 1
  %exitcond1 = icmp ne i32 %19, 100
  br i1 %exitcond1, label %b4, label %b3

b10:
  br i1 false, label %b10, label %b9
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
