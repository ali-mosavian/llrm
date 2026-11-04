target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @bench_recgcd(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync norecurse {
  %2 = icmp slt i32 %0, 1
  br i1 %2, label %19, label %3

3:
  br label %4

4:
  %5 = phi i32 [ 10, %3 ], [ %17, %4 ]
  %6 = phi i32 [ 7, %3 ], [ %16, %4 ]
  %7 = phi i32 [ 91, %3 ], [ %15, %4 ]
  %8 = phi i32 [ %0, %3 ], [ %14, %4 ]
  %9 = phi i32 [ %13, %4 ], [ 0, %3 ]
  %10 = tail call fastcc i32 @gcd(i32 noundef %5, i32 noundef %6) nobuiltin "no-builtins"
  %11 = tail call fastcc i32 @gcd(i32 noundef %7, i32 noundef 360) nobuiltin "no-builtins"
  %12 = add i32 %10, %9
  %13 = add i32 %12, %11
  %14 = add i32 %8, -1
  %15 = add nuw i32 %7, 1
  %16 = add nuw i32 %6, 5
  %17 = add nuw i32 %5, 7
  %18 = icmp eq i32 %14, 0
  br i1 %18, label %19, label %4, !llvm.loop !9

19:
  %20 = phi i32 [ 0, %1 ], [ %13, %4 ]
  ret i32 %20
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal fastcc range(i32 -2147483646, -2147483648) i32 @gcd(i32 noundef range(i32 -2147483646, -2147483648) %0, i32 noundef range(i32 -2147483646, -2147483648) %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync norecurse {
  br label %tailrecurse

tailrecurse:
  %3 = phi i32 [ %0, %2 ], [ %4, %6 ]
  %4 = phi i32 [ %1, %2 ], [ %7, %6 ]
  %5 = icmp eq i32 %4, 0
  br i1 %5, label %8, label %6

6:
  %7 = srem i32 %3, %4
  br label %tailrecurse

8:
  ret i32 %3
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
  br label %1

1:
  %2 = phi i32 [ 10, %0 ], [ %14, %1 ]
  %3 = phi i32 [ 7, %0 ], [ %13, %1 ]
  %4 = phi i32 [ 91, %0 ], [ %12, %1 ]
  %5 = phi i32 [ 300, %0 ], [ %11, %1 ]
  %6 = phi i32 [ %10, %1 ], [ 0, %0 ]
  %7 = tail call fastcc i32 @gcd(i32 noundef %2, i32 noundef %3) nobuiltin "no-builtins"
  %8 = tail call fastcc i32 @gcd(i32 noundef %4, i32 noundef 360) nobuiltin "no-builtins"
  %9 = add i32 %7, %6
  %10 = add i32 %9, %8
  %11 = add nsw i32 %5, -1
  %12 = add nuw nsw i32 %4, 1
  %13 = add nuw nsw i32 %3, 5
  %14 = add nuw nsw i32 %2, 7
  %15 = icmp eq i32 %11, 0
  br i1 %15, label %16, label %1, !llvm.loop !9

16:
  tail call void @report(i32 noundef %10) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext"

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !4, i64 0}
!9 = distinct !{!9, !10, !11}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!"llvm.loop.unroll.disable"}

