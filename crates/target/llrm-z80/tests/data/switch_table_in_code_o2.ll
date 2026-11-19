; ModuleID = 'tests/run/c/switch_table_in_code.c'
source_filename = "tests/run/c/switch_table_in_code.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i32 @weight(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  %4 = load i32, ptr %3, align 2, !tbaa !7
  switch i32 %4, label %17 [
    i32 0, label %5
    i32 1, label %6
    i32 2, label %7
    i32 3, label %8
    i32 4, label %9
    i32 5, label %10
    i32 6, label %11
    i32 7, label %12
    i32 8, label %13
    i32 9, label %14
    i32 10, label %15
    i32 11, label %16
  ]

5:                                                ; preds = %1
  store i32 3, ptr %2, align 2
  br label %18

6:                                                ; preds = %1
  store i32 14, ptr %2, align 2
  br label %18

7:                                                ; preds = %1
  store i32 15, ptr %2, align 2
  br label %18

8:                                                ; preds = %1
  store i32 92, ptr %2, align 2
  br label %18

9:                                                ; preds = %1
  store i32 65, ptr %2, align 2
  br label %18

10:                                               ; preds = %1
  store i32 35, ptr %2, align 2
  br label %18

11:                                               ; preds = %1
  store i32 89, ptr %2, align 2
  br label %18

12:                                               ; preds = %1
  store i32 79, ptr %2, align 2
  br label %18

13:                                               ; preds = %1
  store i32 32, ptr %2, align 2
  br label %18

14:                                               ; preds = %1
  store i32 38, ptr %2, align 2
  br label %18

15:                                               ; preds = %1
  store i32 46, ptr %2, align 2
  br label %18

16:                                               ; preds = %1
  store i32 26, ptr %2, align 2
  br label %18

17:                                               ; preds = %1
  store i32 1, ptr %2, align 2
  br label %18

18:                                               ; preds = %17, %16, %15, %14, %13, %12, %11, %10, %9, %8, %7, %6, %5
  %19 = load i32, ptr %2, align 2
  ret i32 %19
}

; Function Attrs: nounwind
define dso_local i32 @step(i16 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !9
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !9
  switch i16 %5, label %25 [
    i16 0, label %6
    i16 1, label %9
    i16 2, label %12
    i16 3, label %15
    i16 4, label %18
    i16 5, label %22
  ]

6:                                                ; preds = %2
  %7 = load i32, ptr %4, align 2, !tbaa !7
  %8 = add nsw i32 %7, 3
  store i32 %8, ptr %4, align 2, !tbaa !7
  br label %28

9:                                                ; preds = %2
  %10 = load i32, ptr %4, align 2, !tbaa !7
  %11 = mul nsw i32 %10, 2
  store i32 %11, ptr %4, align 2, !tbaa !7
  br label %28

12:                                               ; preds = %2
  %13 = load i32, ptr %4, align 2, !tbaa !7
  %14 = sub nsw i32 %13, 7
  store i32 %14, ptr %4, align 2, !tbaa !7
  br label %28

15:                                               ; preds = %2
  %16 = load i32, ptr %4, align 2, !tbaa !7
  %17 = xor i32 %16, 85
  store i32 %17, ptr %4, align 2, !tbaa !7
  br label %28

18:                                               ; preds = %2
  %19 = load i32, ptr %4, align 2, !tbaa !7
  %20 = shl i32 %19, 1
  %21 = or i32 %20, 1
  store i32 %21, ptr %4, align 2, !tbaa !7
  br label %28

22:                                               ; preds = %2
  %23 = load i32, ptr %4, align 2, !tbaa !7
  %24 = sdiv i32 %23, 3
  store i32 %24, ptr %4, align 2, !tbaa !7
  br label %28

25:                                               ; preds = %2
  %26 = load i32, ptr %4, align 2, !tbaa !7
  %27 = add nsw i32 %26, 1000
  store i32 %27, ptr %4, align 2, !tbaa !7
  br label %28

28:                                               ; preds = %25, %22, %18, %15, %12, %9, %6
  %29 = load i32, ptr %4, align 2, !tbaa !7
  ret i32 %29
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 5, ptr %3, align 2, !tbaa !7
  store i32 -2, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %10, %0
  %5 = load i32, ptr %2, align 2, !tbaa !7
  %6 = icmp slt i32 %5, 15
  br i1 %6, label %7, label %13

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = call i32 @weight(i32 noundef %8)
  call void @report(i32 noundef %9)
  br label %10

10:                                               ; preds = %7
  %11 = load i32, ptr %2, align 2, !tbaa !7
  %12 = add nsw i32 %11, 1
  store i32 %12, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !10

13:                                               ; preds = %4
  store i32 -1, ptr %2, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %23, %13
  %15 = load i32, ptr %2, align 2, !tbaa !7
  %16 = icmp slt i32 %15, 8
  br i1 %16, label %17, label %26

17:                                               ; preds = %14
  %18 = load i32, ptr %2, align 2, !tbaa !7
  %19 = trunc i32 %18 to i16
  %20 = load i32, ptr %3, align 2, !tbaa !7
  %21 = call i32 @step(i16 noundef %19, i32 noundef %20)
  store i32 %21, ptr %3, align 2, !tbaa !7
  %22 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %22)
  br label %23

23:                                               ; preds = %17
  %24 = load i32, ptr %2, align 2, !tbaa !7
  %25 = add nsw i32 %24, 1
  store i32 %25, ptr %2, align 2, !tbaa !7
  br label %14, !llvm.loop !12

26:                                               ; preds = %14
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
