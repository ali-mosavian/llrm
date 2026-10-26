target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_far_a = global [200 x i8] zeroinitializer
@_near_a = global [200 x i8] zeroinitializer
@_near_b = global [200 x i8] zeroinitializer

define i32 @_main() {
b1:
  br label %b4

0:
  %1 = phi i32 [ %13, %4 ], [ 0, %34 ]
  %2 = phi i32 [ %12, %4 ], [ 0, %34 ]
  %3 = trunc nuw nsw i32 %1 to i16
  br label %14

4:
  %5 = getelementptr inbounds nuw [2 x i8], ptr @_far_a, i32 %1
  %6 = load i16, ptr %5, !tbaa !9
  %7 = getelementptr inbounds nuw [2 x i8], ptr @_near_b, i32 %1
  %8 = load i16, ptr %7, !tbaa !9
  %9 = sext i16 %6 to i32
  %10 = sext i16 %8 to i32
  %11 = add i32 %2, %9
  %12 = add i32 %11, %10
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_near_a, ptr align 2 @_near_b, i32 200, i1 false)
  %13 = add nuw nsw i32 %1, 1
  %exitcond1 = icmp ne i32 %13, 100
  br i1 %exitcond1, label %0, label %23

14:
  %15 = phi i32 [ %22, %14 ], [ -200, %0 ]
  %16 = getelementptr i8, ptr @_near_a, i32 %15
  %17 = getelementptr i8, ptr %16, i32 200
  %18 = load i16, ptr %17, !tbaa !9
  %19 = add i16 %18, %3
  %20 = getelementptr i8, ptr @_near_b, i32 %15
  %21 = getelementptr i8, ptr %20, i32 200
  store i16 %19, ptr %21, !tbaa !9
  %22 = add nsw i32 %15, 2
  %exitcond = icmp ne i32 %22, 0
  br i1 %exitcond, label %14, label %4

23:
  %24 = phi i32 [ %12, %4 ]
  %25 = tail call cc 1002 i32 @report_(i32 noundef %24)
  ret i32 0

b4:
  %26 = phi i16 [ 0, %b1 ], [ %33, %b4 ]
  %27 = phi i32 [ 0, %b1 ], [ %32, %b4 ]
  %28 = shl nuw nsw i32 %27, 1
  %29 = getelementptr i8, ptr @_near_a, i32 %28
  %tmp = trunc i32 %27 to i16
  store i16 %tmp, ptr %29, !tbaa !9
  %30 = shl nuw nsw i32 %27, 1
  %31 = getelementptr i8, ptr @_far_a, i32 %30
  store i16 %26, ptr %31, !tbaa !9
  %32 = add nuw nsw i32 %27, 1
  %33 = add nuw nsw i16 %26, 3
  %exitcond2 = icmp ne i32 %32, 100
  br i1 %exitcond2, label %b4, label %34

34:
  br label %0
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
