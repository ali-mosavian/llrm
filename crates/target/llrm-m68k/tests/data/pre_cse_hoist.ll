target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

define fastcc i8 @usng_next_field(ptr %0, i1 %1, i32 %2) {
  %4 = load i32, ptr %0, align 2
  %5 = icmp eq i32 %4, 0
  br i1 %5, label %6, label %13

6:                                                ; preds = %3
  %7 = getelementptr i8, ptr %0, i32 8
  br i1 %1, label %8, label %11

8:                                                ; preds = %6
  %9 = getelementptr i8, ptr %7, i32 %2
  %10 = load i8, ptr %9, align 1
  store i8 %10, ptr %0, align 1
  br label %13

11:                                               ; preds = %6
  %12 = load i8, ptr %7, align 1
  ret i8 %12

13:                                               ; preds = %13, %8, %3
  %14 = getelementptr i8, ptr %0, i32 8
  %15 = load i32, ptr %0, align 2
  %16 = getelementptr i8, ptr %14, i32 %15
  %17 = load i8, ptr %16, align 1
  store i8 %17, ptr %0, align 1
  br label %13
}
