target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_bench_queens_via = global ptr @bench_queens_

define internal fastcc i32 @place_(ptr noundef nonnull nocapture %0, i32 noundef %1, i32 noundef %2) "llrm-fp-eval"="x87-extended" nounwind memory(argmem: readwrite) {
b1:
  %3 = icmp eq i32 %1, %2
  br i1 %3, label %b4, label %b2

b2:
  %4 = icmp sgt i32 %2, 0
  br i1 %4, label %5, label %b4

5:
  %6 = add nsw i32 %1, 1
  %7 = sub i32 0, %1
  br label %9

b4:
  %8 = phi i32 [ 1, %b1 ], [ 0, %b2 ], [ %27, %b8 ]
  ret i32 %8

9:
  %10 = phi i32 [ 0, %5 ], [ %27, %b8 ]
  %11 = phi i32 [ 0, %5 ], [ %28, %b8 ]
  %12 = icmp sgt i32 %1, 0
  br i1 %12, label %13, label %b9

13:
  br label %18

14:
  %15 = add i32 %21, 1
  %16 = add i32 %20, -1
  %17 = getelementptr i8, ptr %19, i32 4
  %exitcond = icmp eq i32 %16, 0
  br i1 %exitcond, label %b9, label %18

18:
  %19 = phi ptr [ %0, %13 ], [ %17, %14 ]
  %20 = phi i32 [ %1, %13 ], [ %16, %14 ]
  %21 = phi i32 [ %7, %13 ], [ %15, %14 ]
  %22 = load i32, ptr %19, !tbaa !11
  %23 = sub nsw i32 %22, %11
  %24 = icmp eq i32 %23, 0
  %25 = icmp eq i32 %20, %23
  %or.cond = select i1 %24, i1 true, i1 %25
  %26 = icmp eq i32 %21, %23
  %or.cond1 = select i1 %or.cond, i1 true, i1 %26
  br i1 %or.cond1, label %b8, label %14

b8:
  %27 = phi i32 [ %32, %b9 ], [ %10, %18 ]
  %28 = add nuw nsw i32 %11, 1
  %exitcond1 = icmp eq i32 %28, %2
  br i1 %exitcond1, label %b4, label %9

b9:
  %29 = mul i32 %1, 4
  %30 = getelementptr i8, ptr %0, i32 %29
  store i32 %11, ptr %30, !tbaa !11
  %31 = tail call fastcc i32 @place_(ptr noundef %0, i32 noundef %6, i32 noundef %2)
  %32 = add nsw i32 %31, %10
  br label %b8
}

define cc 1002 i32 @bench_queens_(i32 noundef %0) "llrm-fp-eval"="x87-extended" nounwind memory(none) {
b1:
  %1 = alloca [48 x i8]
  %2 = call fastcc i32 @place_(ptr noundef %1, i32 noundef 0, i32 noundef %0)
  ret i32 %2
}

define i32 @_main() "llrm-fp-eval"="x87-extended" {
b1:
  %0 = load volatile ptr, ptr @_bench_queens_via, !tbaa !7
  %1 = tail call cc 1002 i32 %0(i32 7)
  %2 = tail call cc 1002 i32 @report_(i32 noundef %1)
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare cc 1002 i32 @report_(i32)

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
