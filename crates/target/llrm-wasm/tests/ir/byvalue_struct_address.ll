; ModuleID = 'byvalue_struct_address.c'
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

%struct.baz = type { i32, i32, i32, i32, i32 }

@sum = internal global i32 0, align 4
@__const.main.x = private unnamed_addr constant %struct.baz { i32 1, i32 2, i32 3, i32 4, i32 5 }, align 4

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden void @bar(ptr noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, i32 noundef %5) #0 {
  %7 = alloca ptr, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  store ptr %0, ptr %7, align 4
  store i32 %1, ptr %8, align 4
  store i32 %2, ptr %9, align 4
  store i32 %3, ptr %10, align 4
  store i32 %4, ptr %11, align 4
  store i32 %5, ptr %12, align 4
  %13 = load ptr, ptr %7, align 4
  %14 = getelementptr inbounds nuw %struct.baz, ptr %13, i32 0, i32 0
  %15 = load i32, ptr %14, align 4
  %16 = mul nsw i32 %15, 1
  %17 = load ptr, ptr %7, align 4
  %18 = getelementptr inbounds nuw %struct.baz, ptr %17, i32 0, i32 1
  %19 = load i32, ptr %18, align 4
  %20 = mul nsw i32 %19, 10
  %21 = add nsw i32 %16, %20
  %22 = load ptr, ptr %7, align 4
  %23 = getelementptr inbounds nuw %struct.baz, ptr %22, i32 0, i32 2
  %24 = load i32, ptr %23, align 4
  %25 = mul nsw i32 %24, 100
  %26 = add nsw i32 %21, %25
  %27 = load ptr, ptr %7, align 4
  %28 = getelementptr inbounds nuw %struct.baz, ptr %27, i32 0, i32 3
  %29 = load i32, ptr %28, align 4
  %30 = mul nsw i32 %29, 1000
  %31 = add nsw i32 %26, %30
  %32 = load ptr, ptr %7, align 4
  %33 = getelementptr inbounds nuw %struct.baz, ptr %32, i32 0, i32 4
  %34 = load i32, ptr %33, align 4
  %35 = mul nsw i32 %34, 10000
  %36 = add nsw i32 %31, %35
  %37 = load i32, ptr %8, align 4
  %38 = add nsw i32 %36, %37
  %39 = load i32, ptr %9, align 4
  %40 = add nsw i32 %38, %39
  %41 = load i32, ptr %10, align 4
  %42 = add nsw i32 %40, %41
  %43 = load i32, ptr %11, align 4
  %44 = add nsw i32 %42, %43
  %45 = load i32, ptr %12, align 4
  %46 = add nsw i32 %44, %45
  store i32 %46, ptr @sum, align 4
  ret void
}

; Function Attrs: nounwind
define hidden void @foo(ptr noundef %0, ptr noundef byval(%struct.baz) align 4 %1, ptr noundef %2) #0 {
  %4 = alloca ptr, align 4
  %5 = alloca ptr, align 4
  store ptr %0, ptr %4, align 4
  store ptr %2, ptr %5, align 4
  call void @bar(ptr noundef %1, i32 noundef 6, i32 noundef 7, i32 noundef 8, i32 noundef 9, i32 noundef 10) #3
  ret void
}

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  %1 = alloca %struct.baz, align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %1, ptr align 4 @__const.main.x, i32 20, i1 false)
  call void @foo(ptr noundef null, ptr noundef byval(%struct.baz) align 4 %1, ptr noundef null) #3
  %2 = load i32, ptr @sum, align 4
  call void @report(i32 noundef %2) #3
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #1

declare void @report(i32 noundef) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nobuiltin "no-builtins" }


