target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

define internal cc 1002 i32 @walk_(ptr noundef nocapture readonly %0) memory(readwrite, argmem: read, inaccessiblemem: none) nounwind norecurse willreturn {
  unreachable
}

define i32 @_main() {
b1:
  br label %b4

0:
  br label %1

1:
  %2 = phi i32 [ 0, %0 ], [ %14, %5 ]
  %3 = phi i32 [ 0, %0 ], [ %13, %5 ]
  %4 = trunc nuw nsw i32 %2 to i16
  br label %15

5:
  %6 = getelementptr inbounds nuw [2 x i8], ptr @_far_a, i32 %2
  %7 = load i16, ptr %6, !tbaa !9
  %8 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %2
  %9 = load i16, ptr %8, !tbaa !9
  %10 = sext i16 %7 to i32
  %11 = sext i16 %9 to i32
  %12 = add i32 %3, %10
  %13 = add i32 %12, %11
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_near_a, ptr align 2 @_near_b, i32 200, i1 false)
  %14 = add nuw nsw i32 %2, 1
  %exitcond1 = icmp ne i32 %14, 100
  br i1 %exitcond1, label %1, label %22

15:
  %16 = phi i32 [ 0, %1 ], [ %21, %15 ]
  %17 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %16
  %18 = load i16, ptr %17, !tbaa !9
  %19 = add i16 %18, %4
  %20 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %16
  store i16 %19, ptr %20, !tbaa !9
  %21 = add nuw nsw i32 %16, 1
  %exitcond = icmp ne i32 %21, 100
  br i1 %exitcond, label %15, label %5

22:
  %23 = tail call cc 1002 i32 @report_(i32 noundef %13)
  ret i32 0

b4:
  %24 = phi i32 [ 0, %b1 ], [ %30, %b4 ]
  %25 = getelementptr inbounds nuw [2 x i8], ptr @_near_a, i32 %24
  %26 = trunc nuw nsw i32 %24 to i16
  store i16 %26, ptr %25, !tbaa !9
  %27 = getelementptr inbounds nuw [2 x i8], ptr @_far_a, i32 %24
  %28 = trunc i32 %24 to i16
  %29 = mul i16 %28, 3
  store i16 %29, ptr %27, !tbaa !9
  %30 = add nuw nsw i32 %24, 1
  %exitcond2 = icmp ne i32 %30, 100
  br i1 %exitcond2, label %b4, label %0
}

declare cc 1002 i32 @report_(i32)

declare void @llvm.memcpy.p0.p0.i32(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

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
