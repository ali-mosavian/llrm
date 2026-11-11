target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@main.numerators = internal constant [7 x i64] [i64 1, i64 -1, i64 100000000000, i64 -100000000000, i64 81985529216486895, i64 7, i64 -7], align 2
@main.divisors = internal constant [6 x i64] [i64 2, i64 -3, i64 1000, i64 4294967296, i64 -4294967297, i64 123456789012], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca [4 x i32], align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i64, align 2
  %6 = alloca i64, align 2
  %7 = alloca i64, align 2
  %8 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.memset.p0.i16(ptr align 2 %2, i8 0, i16 16, i1 false)
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %100, %0
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 7
  br i1 %11, label %12, label %103

12:                                               ; preds = %9
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %13

13:                                               ; preds = %96, %12
  %14 = load i16, ptr %4, align 2, !tbaa !7
  %15 = icmp slt i16 %14, 6
  br i1 %15, label %16, label %99

16:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(ptr %5) #4
  %17 = load i16, ptr %3, align 2, !tbaa !7
  %18 = getelementptr inbounds [7 x i64], ptr @main.numerators, i16 0, i16 %17
  %19 = load i64, ptr %18, align 2, !tbaa !8
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = getelementptr inbounds [6 x i64], ptr @main.divisors, i16 0, i16 %20
  %22 = load i64, ptr %21, align 2, !tbaa !8
  %23 = call i64 @quotient(i64 noundef %19, i64 noundef %22) #5
  store i64 %23, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %24 = load i16, ptr %3, align 2, !tbaa !7
  %25 = getelementptr inbounds [7 x i64], ptr @main.numerators, i16 0, i16 %24
  %26 = load i64, ptr %25, align 2, !tbaa !8
  %27 = load i16, ptr %4, align 2, !tbaa !7
  %28 = getelementptr inbounds [6 x i64], ptr @main.divisors, i16 0, i16 %27
  %29 = load i64, ptr %28, align 2, !tbaa !8
  %30 = call i64 @modulo(i64 noundef %26, i64 noundef %29) #5
  store i64 %30, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %7) #4
  %31 = load i16, ptr %3, align 2, !tbaa !7
  %32 = getelementptr inbounds [7 x i64], ptr @main.numerators, i16 0, i16 %31
  %33 = load i64, ptr %32, align 2, !tbaa !8
  %34 = load i16, ptr %4, align 2, !tbaa !7
  %35 = getelementptr inbounds [6 x i64], ptr @main.divisors, i16 0, i16 %34
  %36 = load i64, ptr %35, align 2, !tbaa !8
  %37 = and i64 %36, 1099511627775
  %38 = call i64 @uquotient(i64 noundef %33, i64 noundef %37) #5
  store i64 %38, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %8) #4
  %39 = load i16, ptr %3, align 2, !tbaa !7
  %40 = getelementptr inbounds [7 x i64], ptr @main.numerators, i16 0, i16 %39
  %41 = load i64, ptr %40, align 2, !tbaa !8
  %42 = load i16, ptr %4, align 2, !tbaa !7
  %43 = getelementptr inbounds [6 x i64], ptr @main.divisors, i16 0, i16 %42
  %44 = load i64, ptr %43, align 2, !tbaa !8
  %45 = and i64 %44, 1099511627775
  %46 = or i64 %45, 1
  %47 = call i64 @umodulo(i64 noundef %41, i64 noundef %46) #5
  store i64 %47, ptr %8, align 2, !tbaa !8
  %48 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 0
  %49 = load i32, ptr %48, align 2, !tbaa !10
  %50 = mul i32 %49, 31
  %51 = load i64, ptr %5, align 2, !tbaa !8
  %52 = trunc i64 %51 to i32
  %53 = add i32 %50, %52
  %54 = load i64, ptr %5, align 2, !tbaa !8
  %55 = ashr i64 %54, 32
  %56 = trunc i64 %55 to i32
  %57 = mul i32 %56, 7
  %58 = add i32 %53, %57
  %59 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 0
  store i32 %58, ptr %59, align 2, !tbaa !10
  %60 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 1
  %61 = load i32, ptr %60, align 2, !tbaa !10
  %62 = mul i32 %61, 31
  %63 = load i64, ptr %6, align 2, !tbaa !8
  %64 = trunc i64 %63 to i32
  %65 = add i32 %62, %64
  %66 = load i64, ptr %6, align 2, !tbaa !8
  %67 = ashr i64 %66, 32
  %68 = trunc i64 %67 to i32
  %69 = mul i32 %68, 7
  %70 = add i32 %65, %69
  %71 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 1
  store i32 %70, ptr %71, align 2, !tbaa !10
  %72 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 2
  %73 = load i32, ptr %72, align 2, !tbaa !10
  %74 = mul i32 %73, 31
  %75 = load i64, ptr %7, align 2, !tbaa !8
  %76 = trunc i64 %75 to i32
  %77 = add i32 %74, %76
  %78 = load i64, ptr %7, align 2, !tbaa !8
  %79 = lshr i64 %78, 32
  %80 = trunc i64 %79 to i32
  %81 = mul i32 %80, 7
  %82 = add i32 %77, %81
  %83 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 2
  store i32 %82, ptr %83, align 2, !tbaa !10
  %84 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 3
  %85 = load i32, ptr %84, align 2, !tbaa !10
  %86 = mul i32 %85, 31
  %87 = load i64, ptr %8, align 2, !tbaa !8
  %88 = trunc i64 %87 to i32
  %89 = add i32 %86, %88
  %90 = load i64, ptr %8, align 2, !tbaa !8
  %91 = lshr i64 %90, 32
  %92 = trunc i64 %91 to i32
  %93 = mul i32 %92, 7
  %94 = add i32 %89, %93
  %95 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 3
  store i32 %94, ptr %95, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  br label %96

96:                                               ; preds = %16
  %97 = load i16, ptr %4, align 2, !tbaa !7
  %98 = add nsw i16 %97, 1
  store i16 %98, ptr %4, align 2, !tbaa !7
  br label %13, !llvm.loop !12

99:                                               ; preds = %13
  br label %100

100:                                              ; preds = %99
  %101 = load i16, ptr %3, align 2, !tbaa !7
  %102 = add nsw i16 %101, 1
  store i16 %102, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !14

103:                                              ; preds = %9
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %104

104:                                              ; preds = %111, %103
  %105 = load i16, ptr %3, align 2, !tbaa !7
  %106 = icmp slt i16 %105, 4
  br i1 %106, label %107, label %114

107:                                              ; preds = %104
  %108 = load i16, ptr %3, align 2, !tbaa !7
  %109 = getelementptr inbounds [4 x i32], ptr %2, i16 0, i16 %108
  %110 = load i32, ptr %109, align 2, !tbaa !10
  call void @report(i32 noundef %110) #5
  br label %111

111:                                              ; preds = %107
  %112 = load i16, ptr %3, align 2, !tbaa !7
  %113 = add nsw i16 %112, 1
  store i16 %113, ptr %3, align 2, !tbaa !7
  br label %104, !llvm.loop !15

114:                                              ; preds = %104
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i16(ptr writeonly captures(none), i8, i16, i1 immarg) #2

; Function Attrs: nounwind optsize
define internal i64 @quotient(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !8
  store i64 %1, ptr %4, align 2, !tbaa !8
  %5 = load i64, ptr %3, align 2, !tbaa !8
  %6 = load i64, ptr %4, align 2, !tbaa !8
  %7 = sdiv i64 %5, %6
  ret i64 %7
}

; Function Attrs: nounwind optsize
define internal i64 @modulo(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !8
  store i64 %1, ptr %4, align 2, !tbaa !8
  %5 = load i64, ptr %3, align 2, !tbaa !8
  %6 = load i64, ptr %4, align 2, !tbaa !8
  %7 = srem i64 %5, %6
  ret i64 %7
}

; Function Attrs: nounwind optsize
define internal i64 @uquotient(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !8
  store i64 %1, ptr %4, align 2, !tbaa !8
  %5 = load i64, ptr %3, align 2, !tbaa !8
  %6 = load i64, ptr %4, align 2, !tbaa !8
  %7 = udiv i64 %5, %6
  ret i64 %7
}

; Function Attrs: nounwind optsize
define internal i64 @umodulo(i64 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i64, align 2
  store i64 %0, ptr %3, align 2, !tbaa !8
  store i64 %1, ptr %4, align 2, !tbaa !8
  %5 = load i64, ptr %3, align 2, !tbaa !8
  %6 = load i64, ptr %4, align 2, !tbaa !8
  %7 = urem i64 %5, %6
  ret i64 %7
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long long", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
