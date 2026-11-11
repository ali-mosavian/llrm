target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @classify(i64 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !7
  %4 = load i64, ptr %3, align 2, !tbaa !7
  switch i64 %4, label %9 [
    i64 3, label %5
    i64 10, label %5
    i64 4294967299, label %6
    i64 -1, label %7
    i64 47, label %8
  ]

5:                                                ; preds = %1, %1
  store i16 1, ptr %2, align 2
  br label %15

6:                                                ; preds = %1
  store i16 2, ptr %2, align 2
  br label %15

7:                                                ; preds = %1
  store i16 3, ptr %2, align 2
  br label %15

8:                                                ; preds = %1
  store i16 4, ptr %2, align 2
  br label %15

9:                                                ; preds = %1
  %10 = load i64, ptr %3, align 2, !tbaa !7
  %11 = ashr i64 %10, 32
  %12 = trunc i64 %11 to i16
  %13 = mul nsw i16 %12, 100
  %14 = add nsw i16 %13, 5
  store i16 %14, ptr %2, align 2
  br label %15

15:                                               ; preds = %9, %8, %7, %6, %5
  %16 = load i16, ptr %2, align 2
  ret i16 %16
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i16 @classify(i64 noundef 3) #2
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #2
  %4 = call i16 @classify(i64 noundef 10) #2
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5) #2
  %6 = call i16 @classify(i64 noundef 4294967299) #2
  %7 = sext i16 %6 to i32
  call void @report(i32 noundef %7) #2
  %8 = call i16 @classify(i64 noundef -1) #2
  %9 = sext i16 %8 to i32
  call void @report(i32 noundef %9) #2
  %10 = call i16 @classify(i64 noundef 47) #2
  %11 = sext i16 %10 to i32
  call void @report(i32 noundef %11) #2
  %12 = call i16 @classify(i64 noundef 8589934595) #2
  %13 = sext i16 %12 to i32
  call void @report(i32 noundef %13) #2
  %14 = call i16 @classify(i64 noundef 0) #2
  %15 = sext i16 %14 to i32
  call void @report(i32 noundef %15) #2
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
!7 = !{!8, !8, i64 0}
!8 = !{!"long long", !5, i64 0}
