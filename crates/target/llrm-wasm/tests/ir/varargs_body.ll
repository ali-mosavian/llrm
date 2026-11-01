; ModuleID = 'va.c'

; Function Attrs: noinline nounwind optnone
define i32 @sum(i32 noundef %0, ...) {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %4, align 4
  call void @llvm.va_start.p0(ptr %3)
  store i32 0, ptr %5, align 4
  br label %6

6:                                                ; preds = %16, %1
  %7 = load i32, ptr %5, align 4
  %8 = load i32, ptr %2, align 4
  %9 = icmp slt i32 %7, %8
  br i1 %9, label %10, label %19

10:                                               ; preds = %6
  %11 = load ptr, ptr %3, align 4
  %12 = getelementptr inbounds i8, ptr %11, i32 4
  store ptr %12, ptr %3, align 4
  %13 = load i32, ptr %11, align 4
  %14 = load i32, ptr %4, align 4
  %15 = add nsw i32 %14, %13
  store i32 %15, ptr %4, align 4
  br label %16

16:                                               ; preds = %10
  %17 = load i32, ptr %5, align 4
  %18 = add nsw i32 %17, 1
  store i32 %18, ptr %5, align 4
  br label %6

19:                                               ; preds = %6
  call void @llvm.va_end.p0(ptr %3)
  %20 = load i32, ptr %4, align 4
  ret i32 %20
}

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_start.p0(ptr)

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_end.p0(ptr)

; Function Attrs: noinline nounwind optnone

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.va_copy.p0(ptr, ptr)

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare ptr @llvm.ptrmask.p0.i32(ptr, i32)

; Function Attrs: noinline nounwind optnone
define i32 @wrap(ptr noundef %0, ...) {
  %2 = alloca ptr, align 4
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  store ptr %0, ptr %2, align 4
  call void @llvm.va_start.p0(ptr %3)
  %5 = load ptr, ptr %2, align 4
  %6 = load ptr, ptr %3, align 4
  %7 = call i32 @forward(ptr noundef %5, ptr noundef %6)
  store i32 %7, ptr %4, align 4
  call void @llvm.va_end.p0(ptr %3)
  %8 = load i32, ptr %4, align 4
  ret i32 %8
}

declare i32 @forward(ptr noundef, ptr noundef)

; Function Attrs: noinline nounwind optnone
define i32 @copied(i32 noundef %0, ...) {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 4
  %4 = alloca ptr, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  store i32 0, ptr %5, align 4
  call void @llvm.va_start.p0(ptr %3)
  call void @llvm.va_copy.p0(ptr %4, ptr %3)
  %6 = load ptr, ptr %3, align 4
  %7 = getelementptr inbounds i8, ptr %6, i32 4
  store ptr %7, ptr %3, align 4
  %8 = load i32, ptr %6, align 4
  %9 = load i32, ptr %5, align 4
  %10 = add nsw i32 %9, %8
  store i32 %10, ptr %5, align 4
  %11 = load ptr, ptr %4, align 4
  %12 = getelementptr inbounds i8, ptr %11, i32 4
  store ptr %12, ptr %4, align 4
  %13 = load i32, ptr %11, align 4
  %14 = load i32, ptr %5, align 4
  %15 = add nsw i32 %14, %13
  store i32 %15, ptr %5, align 4
  call void @llvm.va_end.p0(ptr %4)
  call void @llvm.va_end.p0(ptr %3)
  %16 = load i32, ptr %5, align 4
  ret i32 %16
}
