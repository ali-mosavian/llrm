; ModuleID = 'bench/switch/switch.c'
source_filename = "bench/switch/switch.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

; Function Attrs: nofree norecurse nosync nounwind optsize memory(none)
define dso_local range(i32 2, 1) i32 @bench_switch(i16 noundef %0) local_unnamed_addr #0 {
  %2 = icmp sgt i16 %0, 0
  br i1 %2, label %3, label %41

3:                                                ; preds = %1, %35
  %4 = phi i32 [ %9, %35 ], [ 1, %1 ]
  %5 = phi i16 [ %39, %35 ], [ 0, %1 ]
  %6 = phi i32 [ %38, %35 ], [ 0, %1 ]
  %7 = mul nsw i32 %4, 75
  %8 = add nsw i32 %7, 74
  %9 = srem i32 %8, 65537
  %10 = srem i32 %9, 10
  %11 = srem i32 %9, 14
  switch i32 %11, label %23 [
    i32 0, label %24
    i32 1, label %12
    i32 2, label %13
    i32 3, label %14
    i32 4, label %15
    i32 5, label %16
    i32 6, label %17
    i32 7, label %18
    i32 8, label %19
    i32 9, label %20
    i32 10, label %21
    i32 11, label %22
  ]

12:                                               ; preds = %3
  br label %24

13:                                               ; preds = %3
  br label %24

14:                                               ; preds = %3
  br label %24

15:                                               ; preds = %3
  br label %24

16:                                               ; preds = %3
  br label %24

17:                                               ; preds = %3
  br label %24

18:                                               ; preds = %3
  br label %24

19:                                               ; preds = %3
  br label %24

20:                                               ; preds = %3
  br label %24

21:                                               ; preds = %3
  br label %24

22:                                               ; preds = %3
  br label %24

23:                                               ; preds = %3
  br label %24

24:                                               ; preds = %3, %12, %13, %14, %15, %16, %17, %18, %19, %20, %21, %22, %23
  %25 = phi i32 [ 1, %23 ], [ 26, %22 ], [ 14, %12 ], [ 15, %13 ], [ 92, %14 ], [ 65, %15 ], [ 35, %16 ], [ 89, %17 ], [ 79, %18 ], [ 32, %19 ], [ 38, %20 ], [ 46, %21 ], [ 3, %3 ]
  %26 = mul nsw i32 %10, 37
  %27 = mul nsw i32 %26, %10
  switch i32 %27, label %34 [
    i32 0, label %35
    i32 37, label %28
    i32 148, label %29
    i32 333, label %30
    i32 592, label %31
    i32 925, label %32
    i32 1332, label %33
  ]

28:                                               ; preds = %24
  br label %35

29:                                               ; preds = %24
  br label %35

30:                                               ; preds = %24
  br label %35

31:                                               ; preds = %24
  br label %35

32:                                               ; preds = %24
  br label %35

33:                                               ; preds = %24
  br label %35

34:                                               ; preds = %24
  br label %35

35:                                               ; preds = %24, %28, %29, %30, %31, %32, %33, %34
  %36 = phi i32 [ 1, %34 ], [ 13, %33 ], [ 7, %28 ], [ 23, %29 ], [ 5, %30 ], [ 19, %31 ], [ 2, %32 ], [ 11, %24 ]
  %37 = add nuw nsw i32 %25, %6
  %38 = add nuw nsw i32 %37, %36
  %39 = add nuw nsw i16 %5, 1
  %40 = icmp eq i16 %39, %0
  br i1 %40, label %41, label %3, !llvm.loop !7

41:                                               ; preds = %35, %1
  %42 = phi i32 [ 0, %1 ], [ %38, %35 ]
  ret i32 %42
}

; Function Attrs: nounwind optsize
define dso_local noundef i16 @main() local_unnamed_addr #1 {
  %1 = tail call i32 @bench_switch(i16 noundef 2000) #3
  tail call void @report(i32 noundef %1) #4
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nofree norecurse nosync nounwind optsize memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize }
attributes #4 = { nounwind optsize }

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
!7 = distinct !{!7, !8}
!8 = !{!"llvm.loop.mustprogress"}
