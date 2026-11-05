target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

define dso_local i32 @sum(i32 noundef %0, ...) {
  %2 = alloca i32, align 2
  %3 = alloca ptr, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  call void @llvm.lifetime.start.p0(ptr %3)
  call void @llvm.lifetime.start.p0(ptr %4)
  store i32 0, ptr %4, align 2
  call void @llvm.va_start.p0(ptr %3)
  br label %6

6:                                                ; preds = %10, %1
  %7 = load i32, ptr %2, align 2
  %8 = add nsw i32 %7, -1
  store i32 %8, ptr %2, align 2
  %9 = icmp sgt i32 %7, 0
  br i1 %9, label %10, label %15

10:                                               ; preds = %6
  %11 = va_arg ptr %3, i32
  store i32 %11, ptr %5, align 2
  %12 = load i32, ptr %5, align 2
  %13 = load i32, ptr %4, align 2
  %14 = add nsw i32 %13, %12
  store i32 %14, ptr %4, align 2
  br label %6

15:                                               ; preds = %6
  call void @llvm.va_end.p0(ptr %3)
  %16 = load i32, ptr %4, align 2
  call void @llvm.lifetime.end.p0(ptr %4)
  call void @llvm.lifetime.end.p0(ptr %3)
  ret i32 %16
}

declare void @llvm.lifetime.start.p0(ptr captures(none))
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare void @llvm.lifetime.end.p0(ptr captures(none))
