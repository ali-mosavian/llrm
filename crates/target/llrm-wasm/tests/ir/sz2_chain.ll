; ModuleID = '/home/alim/scratch/bs2-wt/tests/run/c/call_keeps.c'
source_filename = "/home/alim/scratch/bs2-wt/tests/run/c/call_keeps.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  %1 = call i32 @hold(i32 noundef 1) #3
  call void @report(i32 noundef %1) #3
  %2 = call i32 @hold(i32 noundef 77) #3
  call void @report(i32 noundef %2) #3
  %3 = call i32 @hold(i32 noundef 65535) #3
  call void @report(i32 noundef %3) #3
  ret i32 0
}

declare void @report(i32 noundef) #1

; Function Attrs: nounwind
define internal i32 @hold(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %3) #4
  %9 = load i32, ptr %2, align 4, !tbaa !6
  %10 = add i32 305419896, %9
  store i32 %10, ptr %3, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %4) #4
  %11 = load i32, ptr %2, align 4, !tbaa !6
  %12 = xor i32 -1698898192, %11
  store i32 %12, ptr %4, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %5) #4
  %13 = load i32, ptr %2, align 4, !tbaa !6
  %14 = sub i32 -19088744, %13
  store i32 %14, ptr %5, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %15 = load i32, ptr %2, align 4, !tbaa !6
  %16 = mul i32 %15, 3
  %17 = add i32 -2147483647, %16
  store i32 %17, ptr %6, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %7) #4
  %18 = load i32, ptr %3, align 4, !tbaa !6
  %19 = load i32, ptr %4, align 4, !tbaa !6
  %20 = load i32, ptr %5, align 4, !tbaa !6
  %21 = load i32, ptr %6, align 4, !tbaa !6
  %22 = call i32 @churn(i32 noundef %18, i32 noundef %19, i32 noundef %20, i32 noundef %21) #3
  store i32 %22, ptr %7, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %8) #4
  %23 = load i32, ptr %4, align 4, !tbaa !6
  %24 = load i32, ptr %5, align 4, !tbaa !6
  %25 = load i32, ptr %6, align 4, !tbaa !6
  %26 = load i32, ptr %3, align 4, !tbaa !6
  %27 = load i32, ptr %7, align 4, !tbaa !6
  %28 = add i32 %26, %27
  %29 = call i32 @churn(i32 noundef %23, i32 noundef %24, i32 noundef %25, i32 noundef %28) #3
  store i32 %29, ptr %8, align 4, !tbaa !6
  %30 = load i32, ptr %3, align 4, !tbaa !6
  %31 = load i32, ptr %4, align 4, !tbaa !6
  %32 = xor i32 %30, %31
  %33 = load i32, ptr %5, align 4, !tbaa !6
  %34 = load i32, ptr %6, align 4, !tbaa !6
  %35 = xor i32 %33, %34
  %36 = add i32 %32, %35
  %37 = load i32, ptr %7, align 4, !tbaa !6
  %38 = add i32 %36, %37
  %39 = load i32, ptr %8, align 4, !tbaa !6
  %40 = add i32 %38, %39
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i32 %40
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nounwind
define internal i32 @churn(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3) #0 {
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca i32, align 4
  %14 = alloca i32, align 4
  store i32 %0, ptr %5, align 4, !tbaa !6
  store i32 %1, ptr %6, align 4, !tbaa !6
  store i32 %2, ptr %7, align 4, !tbaa !6
  store i32 %3, ptr %8, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %9) #4
  %15 = load i32, ptr %5, align 4, !tbaa !6
  %16 = mul i32 %15, 3
  %17 = load i32, ptr %6, align 4, !tbaa !6
  %18 = add i32 %16, %17
  store i32 %18, ptr %9, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %10) #4
  %19 = load i32, ptr %6, align 4, !tbaa !6
  %20 = mul i32 %19, 5
  %21 = load i32, ptr %7, align 4, !tbaa !6
  %22 = add i32 %20, %21
  store i32 %22, ptr %10, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %11) #4
  %23 = load i32, ptr %7, align 4, !tbaa !6
  %24 = mul i32 %23, 7
  %25 = load i32, ptr %8, align 4, !tbaa !6
  %26 = add i32 %24, %25
  store i32 %26, ptr %11, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %12) #4
  %27 = load i32, ptr %8, align 4, !tbaa !6
  %28 = mul i32 %27, 11
  %29 = load i32, ptr %5, align 4, !tbaa !6
  %30 = add i32 %28, %29
  store i32 %30, ptr %12, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %31 = load i32, ptr %5, align 4, !tbaa !6
  %32 = load i32, ptr %8, align 4, !tbaa !6
  %33 = xor i32 %31, %32
  store i32 %33, ptr %13, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %14) #4
  %34 = load i32, ptr %6, align 4, !tbaa !6
  %35 = load i32, ptr %7, align 4, !tbaa !6
  %36 = xor i32 %34, %35
  store i32 %36, ptr %14, align 4, !tbaa !6
  %37 = load i32, ptr %9, align 4, !tbaa !6
  %38 = shl i32 %37, 3
  %39 = load i32, ptr %10, align 4, !tbaa !6
  %40 = lshr i32 %39, 2
  %41 = xor i32 %38, %40
  %42 = load i32, ptr %13, align 4, !tbaa !6
  %43 = xor i32 %41, %42
  store i32 %43, ptr %9, align 4, !tbaa !6
  %44 = load i32, ptr %10, align 4, !tbaa !6
  %45 = shl i32 %44, 5
  %46 = load i32, ptr %11, align 4, !tbaa !6
  %47 = lshr i32 %46, 3
  %48 = xor i32 %45, %47
  %49 = load i32, ptr %14, align 4, !tbaa !6
  %50 = xor i32 %48, %49
  store i32 %50, ptr %10, align 4, !tbaa !6
  %51 = load i32, ptr %11, align 4, !tbaa !6
  %52 = shl i32 %51, 7
  %53 = load i32, ptr %12, align 4, !tbaa !6
  %54 = lshr i32 %53, 5
  %55 = xor i32 %52, %54
  %56 = load i32, ptr %9, align 4, !tbaa !6
  %57 = xor i32 %55, %56
  store i32 %57, ptr %11, align 4, !tbaa !6
  %58 = load i32, ptr %12, align 4, !tbaa !6
  %59 = shl i32 %58, 9
  %60 = load i32, ptr %9, align 4, !tbaa !6
  %61 = lshr i32 %60, 7
  %62 = xor i32 %59, %61
  %63 = load i32, ptr %10, align 4, !tbaa !6
  %64 = xor i32 %62, %63
  store i32 %64, ptr %12, align 4, !tbaa !6
  %65 = load i32, ptr %9, align 4, !tbaa !6
  %66 = load i32, ptr %10, align 4, !tbaa !6
  %67 = add i32 %65, %66
  %68 = load i32, ptr %11, align 4, !tbaa !6
  %69 = add i32 %67, %68
  %70 = load i32, ptr %12, align 4, !tbaa !6
  %71 = add i32 %69, %70
  %72 = load i32, ptr %13, align 4, !tbaa !6
  %73 = add i32 %71, %72
  %74 = load i32, ptr %14, align 4, !tbaa !6
  %75 = add i32 %73, %74
  call void @llvm.lifetime.end.p0(ptr %14) #4
  call void @llvm.lifetime.end.p0(ptr %13) #4
  call void @llvm.lifetime.end.p0(ptr %12) #4
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  call void @llvm.lifetime.end.p0(ptr %9) #4
  ret i32 %75
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nobuiltin "no-builtins" }
attributes #4 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"long", !4, i64 0}
