target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@switch.table.0 = private constant [12 x i8] [i8 3, i8 14, i8 15, i8 92, i8 65, i8 35, i8 89, i8 79, i8 32, i8 38, i8 46, i8 26]
@switch.table.1 = private constant [12 x i8] [i8 3, i8 14, i8 15, i8 92, i8 65, i8 35, i8 89, i8 79, i8 32, i8 38, i8 46, i8 26]

@__main_void = hidden alias i32 (), ptr @main

define hidden range(i32 -2147483646, -2147483648) i32 @bench_switch(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync norecurse willreturn {
  %2 = icmp sgt i32 %0, 0
  br i1 %2, label %3, label %31

3:
  br label %4

4:
  %5 = phi i32 [ %0, %3 ], [ %30, %26 ]
  %6 = phi i32 [ %10, %26 ], [ 1, %3 ]
  %7 = phi i32 [ %29, %26 ], [ 0, %3 ]
  %8 = mul nsw i32 %6, 75
  %9 = add nsw i32 %8, 74
  %10 = srem i32 %9, 65537
  %11 = srem i32 %10, 10
  %12 = srem i32 %10, 14
  %13 = icmp ult i32 %12, 12
  br i1 %13, label %switch.lookup, label %15

switch.lookup:
  %14 = getelementptr inbounds nuw i8, ptr @switch.table.0, i32 %12
  %switch.load = load i8, ptr %14
  %switch.ext = zext i8 %switch.load to i32
  br label %15

15:
  %16 = phi i32 [ %switch.ext, %switch.lookup ], [ 1, %4 ]
  %17 = mul nsw i32 %11, 37
  %18 = mul nsw i32 %17, %11
  switch i32 %18, label %25 [
    i32 0, label %26
    i32 37, label %19
    i32 148, label %20
    i32 333, label %21
    i32 592, label %22
    i32 925, label %23
    i32 1332, label %24
  ]

19:
  br label %26

20:
  br label %26

21:
  br label %26

22:
  br label %26

23:
  br label %26

24:
  br label %26

25:
  br label %26

26:
  %27 = phi i32 [ 1, %25 ], [ 7, %19 ], [ 23, %20 ], [ 5, %21 ], [ 19, %22 ], [ 2, %23 ], [ 13, %24 ], [ 11, %15 ]
  %28 = add nsw i32 %16, %7
  %29 = add nsw i32 %28, %27
  %30 = add i32 %5, -1
  %exitcond = icmp eq i32 %30, 0
  br i1 %exitcond, label %31, label %4, !llvm.loop !9

31:
  %32 = phi i32 [ 0, %1 ], [ %29, %26 ]
  ret i32 %32
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
  br label %1

1:
  %2 = phi i32 [ 2000, %0 ], [ %27, %23 ]
  %3 = phi i32 [ %7, %23 ], [ 1, %0 ]
  %4 = phi i32 [ %26, %23 ], [ 0, %0 ]
  %5 = mul nsw i32 %3, 75
  %6 = add nsw i32 %5, 74
  %7 = srem i32 %6, 65537
  %8 = srem i32 %7, 10
  %9 = srem i32 %7, 14
  %10 = icmp ult i32 %9, 12
  br i1 %10, label %switch.lookup, label %12

switch.lookup:
  %11 = getelementptr inbounds nuw i8, ptr @switch.table.1, i32 %9
  %switch.load = load i8, ptr %11
  %switch.ext = zext i8 %switch.load to i32
  br label %12

12:
  %13 = phi i32 [ %switch.ext, %switch.lookup ], [ 1, %1 ]
  %14 = mul nsw i32 %8, 37
  %15 = mul nsw i32 %14, %8
  switch i32 %15, label %22 [
    i32 0, label %23
    i32 37, label %16
    i32 148, label %17
    i32 333, label %18
    i32 592, label %19
    i32 925, label %20
    i32 1332, label %21
  ]

16:
  br label %23

17:
  br label %23

18:
  br label %23

19:
  br label %23

20:
  br label %23

21:
  br label %23

22:
  br label %23

23:
  %24 = phi i32 [ 1, %22 ], [ 7, %16 ], [ 23, %17 ], [ 5, %18 ], [ 19, %19 ], [ 2, %20 ], [ 13, %21 ], [ 11, %12 ]
  %25 = add nsw i32 %13, %4
  %26 = add nsw i32 %25, %24
  %27 = add nsw i32 %2, -1
  %exitcond = icmp eq i32 %27, 0
  br i1 %exitcond, label %28, label %1, !llvm.loop !9

28:
  tail call void @report(i32 noundef %26) nobuiltin "no-builtins"
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
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}

