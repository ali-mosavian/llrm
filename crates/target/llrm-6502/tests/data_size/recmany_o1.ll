target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i32 @bench_recmany(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = call i32 @mix(i32 noundef 1, i32 noundef 2, i32 noundef 3, i32 noundef 4, i32 noundef 5, i16 noundef %3) #2
  ret i32 %4
}

; Function Attrs: nounwind optsize
define internal i32 @mix(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, i16 noundef %5) #0 {
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca i32, align 2
  %13 = alloca i16, align 2
  store i32 %0, ptr %8, align 2, !tbaa !8
  store i32 %1, ptr %9, align 2, !tbaa !8
  store i32 %2, ptr %10, align 2, !tbaa !8
  store i32 %3, ptr %11, align 2, !tbaa !8
  store i32 %4, ptr %12, align 2, !tbaa !8
  store i16 %5, ptr %13, align 2, !tbaa !7
  %14 = load i16, ptr %13, align 2, !tbaa !7
  %15 = icmp eq i16 %14, 0
  br i1 %15, label %16, label %26

16:                                               ; preds = %6
  %17 = load i32, ptr %8, align 2, !tbaa !8
  %18 = load i32, ptr %9, align 2, !tbaa !8
  %19 = add nsw i32 %17, %18
  %20 = load i32, ptr %10, align 2, !tbaa !8
  %21 = add nsw i32 %19, %20
  %22 = load i32, ptr %11, align 2, !tbaa !8
  %23 = add nsw i32 %21, %22
  %24 = load i32, ptr %12, align 2, !tbaa !8
  %25 = add nsw i32 %23, %24
  store i32 %25, ptr %7, align 2
  br label %47

26:                                               ; preds = %6
  %27 = load i32, ptr %9, align 2, !tbaa !8
  %28 = add nsw i32 %27, 1
  %29 = load i32, ptr %10, align 2, !tbaa !8
  %30 = load i32, ptr %8, align 2, !tbaa !8
  %31 = srem i32 %30, 5
  %32 = add nsw i32 %29, %31
  %33 = load i32, ptr %11, align 2, !tbaa !8
  %34 = load i32, ptr %9, align 2, !tbaa !8
  %35 = xor i32 %33, %34
  %36 = load i32, ptr %12, align 2, !tbaa !8
  %37 = load i32, ptr %10, align 2, !tbaa !8
  %38 = srem i32 %37, 3
  %39 = add nsw i32 %36, %38
  %40 = load i32, ptr %8, align 2, !tbaa !8
  %41 = load i32, ptr %11, align 2, !tbaa !8
  %42 = srem i32 %41, 7
  %43 = add nsw i32 %40, %42
  %44 = load i16, ptr %13, align 2, !tbaa !7
  %45 = sub nsw i16 %44, 1
  %46 = call i32 @mix(i32 noundef %28, i32 noundef %32, i32 noundef %35, i32 noundef %39, i32 noundef %43, i16 noundef %45) #2
  store i32 %46, ptr %7, align 2
  br label %47

47:                                               ; preds = %26, %16
  %48 = load i32, ptr %7, align 2
  ret i32 %48
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_recmany(i16 noundef 1500) #2
  call void @report(i32 noundef %2) #2
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { optsize }

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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
