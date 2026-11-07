target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@a = hidden global [64 x i16] zeroinitializer, align 16

define hidden void @fillw(i16 noundef signext %0, i16 noundef signext %1) {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %3, align 2
  store i16 %1, ptr %4, align 2
  call void @llvm.lifetime.start.p0(ptr %5)
  store i16 0, ptr %5, align 2
  br label %6

6:                                                ; preds = %17, %2
  %7 = load i16, ptr %5, align 2
  %8 = sext i16 %7 to i32
  %9 = load i16, ptr %3, align 2
  %10 = sext i16 %9 to i32
  %11 = icmp slt i32 %8, %10
  br i1 %11, label %12, label %20

12:                                               ; preds = %6
  %13 = load i16, ptr %4, align 2
  %14 = load i16, ptr %5, align 2
  %15 = sext i16 %14 to i32
  %16 = getelementptr inbounds [64 x i16], ptr @a, i32 0, i32 %15
  store i16 %13, ptr %16, align 2
  br label %17

17:                                               ; preds = %12
  %18 = load i16, ptr %5, align 2
  %19 = add i16 %18, 1
  store i16 %19, ptr %5, align 2
  br label %6

20:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5)
  ret void
}

declare void @llvm.lifetime.start.p0(ptr captures(none))

declare void @llvm.lifetime.end.p0(ptr captures(none))



