; ModuleID = 'tests/run/c/recmany_o1.c'
source_filename = "tests/run/c/recmany_o1.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"

; Function Attrs: nofree norecurse nosync nounwind optsize memory(none)
define dso_local i32 @bench_recmany(i16 noundef %0) local_unnamed_addr #0 {
  %2 = icmp eq i16 %0, 0
  br i1 %2, label %25, label %3

3:                                                ; preds = %1, %3
  %4 = phi i16 [ %18, %3 ], [ %0, %1 ]
  %5 = phi i32 [ %17, %3 ], [ 5, %1 ]
  %6 = phi i32 [ %15, %3 ], [ 4, %1 ]
  %7 = phi i32 [ %13, %3 ], [ 3, %1 ]
  %8 = phi i32 [ %12, %3 ], [ 2, %1 ]
  %9 = phi i32 [ %10, %3 ], [ 1, %1 ]
  %10 = add nsw i32 %8, 1
  %11 = srem i32 %9, 5
  %12 = add nsw i32 %11, %7
  %13 = xor i32 %8, %6
  %14 = srem i32 %7, 3
  %15 = add nsw i32 %14, %5
  %16 = srem i32 %6, 7
  %17 = add nsw i32 %9, %16
  %18 = add nsw i16 %4, -1
  %19 = icmp eq i16 %18, 0
  br i1 %19, label %20, label %3

20:                                               ; preds = %3
  %21 = add nsw i32 %12, %10
  %22 = add nsw i32 %21, %13
  %23 = add nsw i32 %22, %15
  %24 = add nsw i32 %23, %17
  br label %25

25:                                               ; preds = %20, %1
  %26 = phi i32 [ 15, %1 ], [ %24, %20 ]
  ret i32 %26
}

; Function Attrs: nounwind optsize
define dso_local noundef i16 @main() local_unnamed_addr #1 {
  br label %1

1:                                                ; preds = %1, %0
  %2 = phi i16 [ %16, %1 ], [ 1500, %0 ]
  %3 = phi i32 [ %15, %1 ], [ 5, %0 ]
  %4 = phi i32 [ %13, %1 ], [ 4, %0 ]
  %5 = phi i32 [ %11, %1 ], [ 3, %0 ]
  %6 = phi i32 [ %10, %1 ], [ 2, %0 ]
  %7 = phi i32 [ %8, %1 ], [ 1, %0 ]
  %8 = add nsw i32 %6, 1
  %9 = srem i32 %7, 5
  %10 = add nsw i32 %9, %5
  %11 = xor i32 %6, %4
  %12 = srem i32 %5, 3
  %13 = add nsw i32 %12, %3
  %14 = srem i32 %4, 7
  %15 = add nsw i32 %7, %14
  %16 = add nsw i16 %2, -1
  %17 = icmp eq i16 %16, 0
  br i1 %17, label %18, label %1

18:                                               ; preds = %1
  %19 = add i32 %11, %8
  %20 = add i32 %19, %13
  %21 = add i32 %20, %15
  %22 = add i32 %21, %10
  tail call void @report(i32 noundef %22) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nofree norecurse nosync nounwind optsize memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
