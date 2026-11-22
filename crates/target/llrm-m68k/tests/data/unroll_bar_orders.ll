; ModuleID = 'unroll_bar_orders.c'
source_filename = "unroll_bar_orders.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@v = internal global [14 x i32] zeroinitializer, align 2

; Function Attrs: noinline nounwind
define dso_local i32 @f(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  store i32 0, ptr %3, align 2
  store i32 0, ptr %4, align 2
  br label %8

8:                                                ; preds = %66, %1
  %9 = load i32, ptr %4, align 2
  %10 = icmp slt i32 %9, 14
  br i1 %10, label %11, label %69

11:                                               ; preds = %8
  %12 = load i32, ptr %4, align 2
  %13 = getelementptr inbounds [14 x i32], ptr @v, i32 0, i32 %12
  %14 = load volatile i32, ptr %13, align 2
  store i32 %14, ptr %5, align 2
  %15 = load i32, ptr %2, align 2
  store i32 %15, ptr %6, align 2
  %16 = load i32, ptr %5, align 2
  store i32 %16, ptr %7, align 2
  %17 = load i32, ptr %3, align 2
  %18 = mul i32 %17, 257
  %19 = load i32, ptr %2, align 2
  %20 = load i32, ptr %5, align 2
  %21 = icmp ult i32 %19, %20
  %22 = zext i1 %21 to i32
  %23 = load i32, ptr %2, align 2
  %24 = load i32, ptr %5, align 2
  %25 = icmp ule i32 %23, %24
  %26 = zext i1 %25 to i32
  %27 = mul nsw i32 2, %26
  %28 = add nsw i32 %22, %27
  %29 = load i32, ptr %2, align 2
  %30 = load i32, ptr %5, align 2
  %31 = icmp ugt i32 %29, %30
  %32 = zext i1 %31 to i32
  %33 = mul nsw i32 4, %32
  %34 = add nsw i32 %28, %33
  %35 = load i32, ptr %2, align 2
  %36 = load i32, ptr %5, align 2
  %37 = icmp uge i32 %35, %36
  %38 = zext i1 %37 to i32
  %39 = mul nsw i32 8, %38
  %40 = add nsw i32 %34, %39
  %41 = load i32, ptr %6, align 2
  %42 = load i32, ptr %7, align 2
  %43 = icmp slt i32 %41, %42
  %44 = zext i1 %43 to i32
  %45 = mul nsw i32 16, %44
  %46 = add nsw i32 %40, %45
  %47 = load i32, ptr %6, align 2
  %48 = load i32, ptr %7, align 2
  %49 = icmp sle i32 %47, %48
  %50 = zext i1 %49 to i32
  %51 = mul nsw i32 32, %50
  %52 = add nsw i32 %46, %51
  %53 = load i32, ptr %6, align 2
  %54 = load i32, ptr %7, align 2
  %55 = icmp sgt i32 %53, %54
  %56 = zext i1 %55 to i32
  %57 = mul nsw i32 64, %56
  %58 = add nsw i32 %52, %57
  %59 = load i32, ptr %6, align 2
  %60 = load i32, ptr %7, align 2
  %61 = icmp sge i32 %59, %60
  %62 = zext i1 %61 to i32
  %63 = mul nsw i32 128, %62
  %64 = add nsw i32 %58, %63
  %65 = add i32 %18, %64
  store i32 %65, ptr %3, align 2
  br label %66

66:                                               ; preds = %11
  %67 = load i32, ptr %4, align 2
  %68 = add nsw i32 %67, 1
  store i32 %68, ptr %4, align 2
  br label %8, !llvm.loop !4

69:                                               ; preds = %8
  %70 = load i32, ptr %3, align 2
  ret i32 %70
}

attributes #0 = { noinline nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = distinct !{!4, !5}
!5 = !{!"llvm.loop.mustprogress"}
