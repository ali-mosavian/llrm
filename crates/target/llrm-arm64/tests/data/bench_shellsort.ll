; ModuleID = '/home/alim/scratch/w15-out-rt20/src/shellsort.c'
source_filename = "/home/alim/scratch/w15-out-rt20/src/shellsort.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i64 @bench_shellsort(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [64 x i16], align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i64, align 8
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  store i16 %0, ptr %2, align 2
  store i64 0, ptr %6, align 8
  store i16 0, ptr %4, align 2
  br label %9

9:                                                ; preds = %30, %1
  %10 = load i16, ptr %4, align 2
  %11 = zext i16 %10 to i32
  %12 = icmp slt i32 %11, 64
  br i1 %12, label %13, label %33

13:                                               ; preds = %9
  %14 = load i16, ptr %4, align 2
  %15 = zext i16 %14 to i64
  %16 = mul i64 %15, 109
  %17 = add i64 %16, 37
  %18 = load i16, ptr %4, align 2
  %19 = zext i16 %18 to i32
  %20 = shl i32 %19, 7
  %21 = sext i32 %20 to i64
  %22 = xor i64 %17, %21
  %23 = load i16, ptr %2, align 2
  %24 = zext i16 %23 to i64
  %25 = xor i64 %22, %24
  %26 = trunc i64 %25 to i16
  %27 = load i16, ptr %4, align 2
  %28 = zext i16 %27 to i64
  %29 = getelementptr inbounds nuw [64 x i16], ptr %3, i64 0, i64 %28
  store i16 %26, ptr %29, align 2
  br label %30

30:                                               ; preds = %13
  %31 = load i16, ptr %4, align 2
  %32 = add i16 %31, 1
  store i16 %32, ptr %4, align 2
  br label %9, !llvm.loop !11

33:                                               ; preds = %9
  store i16 32, ptr %5, align 2
  br label %34

34:                                               ; preds = %97, %33
  %35 = load i16, ptr %5, align 2
  %36 = icmp ne i16 %35, 0
  br i1 %36, label %37, label %102

37:                                               ; preds = %34
  %38 = load i16, ptr %5, align 2
  store i16 %38, ptr %4, align 2
  br label %39

39:                                               ; preds = %93, %37
  %40 = load i16, ptr %4, align 2
  %41 = zext i16 %40 to i32
  %42 = icmp slt i32 %41, 64
  br i1 %42, label %43, label %96

43:                                               ; preds = %39
  %44 = load i16, ptr %4, align 2
  store i16 %44, ptr %7, align 2
  %45 = load i16, ptr %4, align 2
  %46 = zext i16 %45 to i64
  %47 = getelementptr inbounds nuw [64 x i16], ptr %3, i64 0, i64 %46
  %48 = load i16, ptr %47, align 2
  store i16 %48, ptr %8, align 2
  br label %49

49:                                               ; preds = %70, %43
  %50 = load i16, ptr %7, align 2
  %51 = zext i16 %50 to i32
  %52 = load i16, ptr %5, align 2
  %53 = zext i16 %52 to i32
  %54 = icmp sge i32 %51, %53
  br i1 %54, label %55, label %68

55:                                               ; preds = %49
  %56 = load i16, ptr %7, align 2
  %57 = zext i16 %56 to i32
  %58 = load i16, ptr %5, align 2
  %59 = zext i16 %58 to i32
  %60 = sub nsw i32 %57, %59
  %61 = sext i32 %60 to i64
  %62 = getelementptr inbounds [64 x i16], ptr %3, i64 0, i64 %61
  %63 = load i16, ptr %62, align 2
  %64 = zext i16 %63 to i32
  %65 = load i16, ptr %8, align 2
  %66 = zext i16 %65 to i32
  %67 = icmp sgt i32 %64, %66
  br label %68

68:                                               ; preds = %55, %49
  %69 = phi i1 [ false, %49 ], [ %67, %55 ]
  br i1 %69, label %70, label %88

70:                                               ; preds = %68
  %71 = load i16, ptr %7, align 2
  %72 = zext i16 %71 to i32
  %73 = load i16, ptr %5, align 2
  %74 = zext i16 %73 to i32
  %75 = sub nsw i32 %72, %74
  %76 = sext i32 %75 to i64
  %77 = getelementptr inbounds [64 x i16], ptr %3, i64 0, i64 %76
  %78 = load i16, ptr %77, align 2
  %79 = load i16, ptr %7, align 2
  %80 = zext i16 %79 to i64
  %81 = getelementptr inbounds nuw [64 x i16], ptr %3, i64 0, i64 %80
  store i16 %78, ptr %81, align 2
  %82 = load i16, ptr %7, align 2
  %83 = zext i16 %82 to i32
  %84 = load i16, ptr %5, align 2
  %85 = zext i16 %84 to i32
  %86 = sub nsw i32 %83, %85
  %87 = trunc i32 %86 to i16
  store i16 %87, ptr %7, align 2
  br label %49, !llvm.loop !13

88:                                               ; preds = %68
  %89 = load i16, ptr %8, align 2
  %90 = load i16, ptr %7, align 2
  %91 = zext i16 %90 to i64
  %92 = getelementptr inbounds nuw [64 x i16], ptr %3, i64 0, i64 %91
  store i16 %89, ptr %92, align 2
  br label %93

93:                                               ; preds = %88
  %94 = load i16, ptr %4, align 2
  %95 = add i16 %94, 1
  store i16 %95, ptr %4, align 2
  br label %39, !llvm.loop !14

96:                                               ; preds = %39
  br label %97

97:                                               ; preds = %96
  %98 = load i16, ptr %5, align 2
  %99 = zext i16 %98 to i32
  %100 = ashr i32 %99, 1
  %101 = trunc i32 %100 to i16
  store i16 %101, ptr %5, align 2
  br label %34, !llvm.loop !15

102:                                              ; preds = %34
  store i16 0, ptr %4, align 2
  br label %103

103:                                              ; preds = %120, %102
  %104 = load i16, ptr %4, align 2
  %105 = zext i16 %104 to i32
  %106 = icmp slt i32 %105, 64
  br i1 %106, label %107, label %123

107:                                              ; preds = %103
  %108 = load i16, ptr %4, align 2
  %109 = zext i16 %108 to i64
  %110 = getelementptr inbounds nuw [64 x i16], ptr %3, i64 0, i64 %109
  %111 = load i16, ptr %110, align 2
  %112 = zext i16 %111 to i64
  %113 = load i16, ptr %4, align 2
  %114 = zext i16 %113 to i32
  %115 = add nsw i32 %114, 1
  %116 = sext i32 %115 to i64
  %117 = mul i64 %112, %116
  %118 = load i64, ptr %6, align 8
  %119 = add i64 %118, %117
  store i64 %119, ptr %6, align 8
  br label %120

120:                                              ; preds = %107
  %121 = load i16, ptr %4, align 2
  %122 = add i16 %121, 1
  store i16 %122, ptr %4, align 2
  br label %103, !llvm.loop !16

123:                                              ; preds = %103
  %124 = load i64, ptr %6, align 8
  ret i64 %124
}

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile i32, ptr @opq0, align 4
  %3 = add nsw i32 0, %2
  %4 = trunc i32 %3 to i16
  %5 = call i64 @bench_shellsort(i16 noundef %4)
  call void @report(i64 noundef %5)
  ret i32 0
}

declare void @report(i64 noundef) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
