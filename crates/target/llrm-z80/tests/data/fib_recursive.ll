target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i32 @bench_fib(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2
  %3 = load i16, ptr %2, align 2
  %4 = call i16 @fib(i16 noundef %3)
  %5 = sext i16 %4 to i32
  ret i32 %5
}

; Function Attrs: nounwind
define internal i16 @fib(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %3, align 2
  %4 = load i16, ptr %3, align 2
  %5 = icmp slt i16 %4, 2
  br i1 %5, label %6, label %8

6:                                                ; preds = %1
  %7 = load i16, ptr %3, align 2
  store i16 %7, ptr %2, align 2
  br label %16

8:                                                ; preds = %1
  %9 = load i16, ptr %3, align 2
  %10 = sub nsw i16 %9, 1
  %11 = call i16 @fib(i16 noundef %10)
  %12 = load i16, ptr %3, align 2
  %13 = sub nsw i16 %12, 2
  %14 = call i16 @fib(i16 noundef %13)
  %15 = add nsw i16 %11, %14
  store i16 %15, ptr %2, align 2
  br label %16

16:                                               ; preds = %8, %6
  %17 = load i16, ptr %2, align 2
  ret i16 %17
}

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_fib(i16 noundef 20)
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }


