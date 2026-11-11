; ModuleID = '/home/alim/scratch/wt/srem/bench/sieve/sieve.c'
source_filename = "/home/alim/scratch/wt/srem/bench/sieve/sieve.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: noinline nounwind
define dso_local i32 @bench_sieve(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [1024 x i8], align 1
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i32, align 2
  store i16 %0, ptr %2, align 2
  store i16 0, ptr %6, align 2
  store i32 0, ptr %7, align 2
  store i16 0, ptr %4, align 2
  br label %8

8:                                                ; preds = %18, %1
  %9 = load i16, ptr %4, align 2
  %10 = zext i16 %9 to i32
  %11 = load i16, ptr %2, align 2
  %12 = zext i16 %11 to i32
  %13 = icmp slt i32 %10, %12
  br i1 %13, label %14, label %21

14:                                               ; preds = %8
  %15 = load i16, ptr %4, align 2
  %16 = zext i16 %15 to i32
  %17 = getelementptr inbounds nuw [1024 x i8], ptr %3, i32 0, i32 %16
  store i8 0, ptr %17, align 1
  br label %18

18:                                               ; preds = %14
  %19 = load i16, ptr %4, align 2
  %20 = add i16 %19, 1
  store i16 %20, ptr %4, align 2
  br label %8, !llvm.loop !4

21:                                               ; preds = %8
  store i16 2, ptr %4, align 2
  br label %22

22:                                               ; preds = %71, %21
  %23 = load i16, ptr %4, align 2
  %24 = zext i16 %23 to i32
  %25 = load i16, ptr %2, align 2
  %26 = zext i16 %25 to i32
  %27 = icmp slt i32 %24, %26
  br i1 %27, label %28, label %74

28:                                               ; preds = %22
  %29 = load i16, ptr %4, align 2
  %30 = zext i16 %29 to i32
  %31 = getelementptr inbounds nuw [1024 x i8], ptr %3, i32 0, i32 %30
  %32 = load i8, ptr %31, align 1
  %33 = icmp ne i8 %32, 0
  br i1 %33, label %34, label %35

34:                                               ; preds = %28
  br label %71

35:                                               ; preds = %28
  %36 = load i16, ptr %6, align 2
  %37 = add i16 %36, 1
  store i16 %37, ptr %6, align 2
  %38 = load i16, ptr %4, align 2
  %39 = zext i16 %38 to i32
  %40 = load i32, ptr %7, align 2
  %41 = add i32 %40, %39
  store i32 %41, ptr %7, align 2
  %42 = load i16, ptr %4, align 2
  %43 = zext i16 %42 to i32
  %44 = icmp sle i32 %43, 31
  br i1 %44, label %45, label %70

45:                                               ; preds = %35
  %46 = load i16, ptr %4, align 2
  %47 = zext i16 %46 to i32
  %48 = load i16, ptr %4, align 2
  %49 = zext i16 %48 to i32
  %50 = mul nsw i32 %47, %49
  %51 = trunc i32 %50 to i16
  store i16 %51, ptr %5, align 2
  br label %52

52:                                               ; preds = %62, %45
  %53 = load i16, ptr %5, align 2
  %54 = zext i16 %53 to i32
  %55 = load i16, ptr %2, align 2
  %56 = zext i16 %55 to i32
  %57 = icmp slt i32 %54, %56
  br i1 %57, label %58, label %69

58:                                               ; preds = %52
  %59 = load i16, ptr %5, align 2
  %60 = zext i16 %59 to i32
  %61 = getelementptr inbounds nuw [1024 x i8], ptr %3, i32 0, i32 %60
  store i8 1, ptr %61, align 1
  br label %62

62:                                               ; preds = %58
  %63 = load i16, ptr %5, align 2
  %64 = zext i16 %63 to i32
  %65 = load i16, ptr %4, align 2
  %66 = zext i16 %65 to i32
  %67 = add nsw i32 %64, %66
  %68 = trunc i32 %67 to i16
  store i16 %68, ptr %5, align 2
  br label %52, !llvm.loop !6

69:                                               ; preds = %52
  br label %70

70:                                               ; preds = %69, %35
  br label %71

71:                                               ; preds = %70, %34
  %72 = load i16, ptr %4, align 2
  %73 = add i16 %72, 1
  store i16 %73, ptr %4, align 2
  br label %22, !llvm.loop !7

74:                                               ; preds = %22
  %75 = load i16, ptr %6, align 2
  %76 = zext i16 %75 to i32
  %77 = shl i32 %76, 16
  %78 = load i32, ptr %7, align 2
  %79 = xor i32 %77, %78
  ret i32 %79
}

; Function Attrs: noinline nounwind
define dso_local i32 @main() #0 {
  %1 = call i32 @bench_sieve(i16 noundef zeroext 1024) #2
  call void @report(i32 noundef %1) #2
  ret i32 0
}

declare void @report(i32 noundef) #1

attributes #0 = { noinline nounwind "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #2 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = distinct !{!4, !5}
!5 = !{!"llvm.loop.mustprogress"}
!6 = distinct !{!6, !5}
!7 = distinct !{!7, !5}
