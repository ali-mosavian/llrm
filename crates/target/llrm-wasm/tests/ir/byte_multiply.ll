; ModuleID = 'byte_multiply.c'
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  store i32 0, ptr %2, align 4
  store i32 0, ptr %3, align 4
  store i32 0, ptr %4, align 4
  br label %6

6:                                                ; preds = %34, %0
  %7 = load i32, ptr %4, align 4
  %8 = icmp slt i32 %7, 256
  br i1 %8, label %9, label %37

9:                                                ; preds = %6
  store i32 0, ptr %5, align 4
  br label %10

10:                                               ; preds = %30, %9
  %11 = load i32, ptr %5, align 4
  %12 = icmp slt i32 %11, 256
  br i1 %12, label %13, label %33

13:                                               ; preds = %10
  %14 = load i32, ptr %4, align 4
  %15 = trunc i32 %14 to i8
  %16 = load i32, ptr %5, align 4
  %17 = trunc i32 %16 to i8
  %18 = call zeroext i8 @unsigned_product(i8 noundef zeroext %15, i8 noundef zeroext %17) #2
  %19 = zext i8 %18 to i32
  %20 = load i32, ptr %1, align 4
  %21 = add nsw i32 %20, %19
  store i32 %21, ptr %1, align 4
  %22 = load i32, ptr %4, align 4
  %23 = trunc i32 %22 to i8
  %24 = load i32, ptr %5, align 4
  %25 = trunc i32 %24 to i8
  %26 = call signext i8 @signed_product(i8 noundef signext %23, i8 noundef signext %25) #2
  %27 = sext i8 %26 to i32
  %28 = load i32, ptr %2, align 4
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %2, align 4
  br label %30

30:                                               ; preds = %13
  %31 = load i32, ptr %5, align 4
  %32 = add nsw i32 %31, 7
  store i32 %32, ptr %5, align 4
  br label %10, !llvm.loop !1

33:                                               ; preds = %10
  br label %34

34:                                               ; preds = %33
  %35 = load i32, ptr %4, align 4
  %36 = add nsw i32 %35, 5
  store i32 %36, ptr %4, align 4
  br label %6, !llvm.loop !3

37:                                               ; preds = %6
  store i32 0, ptr %4, align 4
  br label %38

38:                                               ; preds = %54, %37
  %39 = load i32, ptr %4, align 4
  %40 = icmp slt i32 %39, 256
  br i1 %40, label %41, label %57

41:                                               ; preds = %38
  %42 = load i32, ptr %4, align 4
  %43 = trunc i32 %42 to i8
  %44 = call zeroext i8 @by_constant(i8 noundef zeroext %43) #2
  %45 = zext i8 %44 to i32
  %46 = mul nsw i32 %45, 3
  %47 = load i32, ptr %4, align 4
  %48 = trunc i32 %47 to i8
  %49 = call signext i8 @by_negative(i8 noundef signext %48) #2
  %50 = sext i8 %49 to i32
  %51 = add nsw i32 %46, %50
  %52 = load i32, ptr %3, align 4
  %53 = add nsw i32 %52, %51
  store i32 %53, ptr %3, align 4
  br label %54

54:                                               ; preds = %41
  %55 = load i32, ptr %4, align 4
  %56 = add nsw i32 %55, 1
  store i32 %56, ptr %4, align 4
  br label %38, !llvm.loop !4

57:                                               ; preds = %38
  %58 = load i32, ptr %1, align 4
  call void @report(i32 noundef %58) #2
  %59 = load i32, ptr %2, align 4
  call void @report(i32 noundef %59) #2
  %60 = load i32, ptr %3, align 4
  call void @report(i32 noundef %60) #2
  ret i32 0
}

; Function Attrs: nounwind
define internal zeroext i8 @unsigned_product(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1
  store i8 %1, ptr %4, align 1
  %5 = load i8, ptr %3, align 1
  %6 = zext i8 %5 to i32
  %7 = load i8, ptr %4, align 1
  %8 = zext i8 %7 to i32
  %9 = mul nsw i32 %6, %8
  %10 = trunc i32 %9 to i8
  ret i8 %10
}

; Function Attrs: nounwind
define internal signext i8 @signed_product(i8 noundef signext %0, i8 noundef signext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1
  store i8 %1, ptr %4, align 1
  %5 = load i8, ptr %3, align 1
  %6 = sext i8 %5 to i32
  %7 = load i8, ptr %4, align 1
  %8 = sext i8 %7 to i32
  %9 = mul nsw i32 %6, %8
  %10 = trunc i32 %9 to i8
  ret i8 %10
}

; Function Attrs: nounwind
define internal zeroext i8 @by_constant(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  store i8 %0, ptr %2, align 1
  %3 = load i8, ptr %2, align 1
  %4 = zext i8 %3 to i32
  %5 = mul nsw i32 %4, 200
  %6 = trunc i32 %5 to i8
  ret i8 %6
}

; Function Attrs: nounwind
define internal signext i8 @by_negative(i8 noundef signext %0) #0 {
  %2 = alloca i8, align 1
  store i8 %0, ptr %2, align 1
  %3 = load i8, ptr %2, align 1
  %4 = sext i8 %3 to i32
  %5 = mul nsw i32 %4, -7
  %6 = trunc i32 %5 to i8
  ret i8 %6
}

declare void @report(i32 noundef) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nobuiltin "no-builtins" }


!1 = distinct !{!1, !2}
!2 = !{!"llvm.loop.mustprogress"}
!3 = distinct !{!3, !2}
!4 = distinct !{!4, !2}
