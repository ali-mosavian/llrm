target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

define dso_local i32 @pick(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %4, align 2
  store i16 %1, ptr %5, align 2
  %6 = load i16, ptr %4, align 2
  switch i16 %6, label %14 [
    i16 192, label %7
    i16 194, label %8
    i16 196, label %9
    i16 219, label %10
    i16 193, label %11
    i16 195, label %11
    i16 197, label %11
    i16 198, label %11
    i16 199, label %11
    i16 200, label %11
    i16 201, label %11
    i16 202, label %11
    i16 203, label %11
    i16 204, label %11
    i16 205, label %11
    i16 206, label %11
    i16 207, label %11
    i16 208, label %11
    i16 209, label %11
    i16 210, label %11
    i16 211, label %11
    i16 212, label %11
    i16 213, label %11
    i16 214, label %11
    i16 215, label %11
    i16 216, label %11
    i16 217, label %11
    i16 218, label %11
  ]

7:                                                ; preds = %2
  store i32 3, ptr %3, align 2
  br label %15

8:                                                ; preds = %2
  store i32 4, ptr %3, align 2
  br label %15

9:                                                ; preds = %2
  store i32 2, ptr %3, align 2
  br label %15

10:                                               ; preds = %2
  store i32 1, ptr %3, align 2
  br label %15

11:                                               ; preds = %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2, %2
  %12 = load i16, ptr %5, align 2
  %13 = sext i16 %12 to i32
  store i32 %13, ptr %3, align 2
  br label %15

14:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  br label %15

15:                                               ; preds = %14, %11, %10, %9, %8, %7
  %16 = load i32, ptr %3, align 2
  ret i32 %16
}

attributes #0 = { minsize nounwind optsize }
