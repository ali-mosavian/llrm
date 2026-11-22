; ModuleID = 'x_ld_matmul.c'
source_filename = "x_ld_matmul.c"
target datalayout = "@m32-datalayout@"

@bench_x_ld_matmul_via = dso_local global ptr @bench_x_ld_matmul, align 4
@A = internal global [16 x [16 x x86_fp80]] zeroinitializer, align 4
@B = internal global [16 x [16 x x86_fp80]] zeroinitializer, align 4
@C = internal global [16 x [16 x x86_fp80]] zeroinitializer, align 4

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_x_ld_matmul(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !4
  %3 = load i32, ptr %2, align 4, !tbaa !4
  %4 = call x86_fp80 @ld_matmul(i32 noundef %3)
  %5 = fptosi x86_fp80 %4 to i32
  ret i32 %5
}

; Function Attrs: nounwind uwtable
define internal x86_fp80 @ld_matmul(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca x86_fp80, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca x86_fp80, align 4
  %10 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !4
  call void @llvm.lifetime.start.p0(i64 12, ptr %3) #4
  store x86_fp80 0xK00000000000000000000, ptr %3, align 4, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 4, ptr %4) #4
  store i32 0, ptr %4, align 4, !tbaa !4
  br label %11

11:                                               ; preds = %45, %1
  %12 = load i32, ptr %4, align 4, !tbaa !4
  %13 = load i32, ptr %2, align 4, !tbaa !4
  %14 = icmp slt i32 %12, %13
  br i1 %14, label %16, label %15

15:                                               ; preds = %11
  store i32 2, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 4, ptr %4) #4
  br label %48

16:                                               ; preds = %11
  call void @llvm.lifetime.start.p0(i64 4, ptr %6) #4
  store i32 0, ptr %6, align 4, !tbaa !4
  br label %17

17:                                               ; preds = %41, %16
  %18 = load i32, ptr %6, align 4, !tbaa !4
  %19 = load i32, ptr %2, align 4, !tbaa !4
  %20 = icmp slt i32 %18, %19
  br i1 %20, label %22, label %21

21:                                               ; preds = %17
  store i32 5, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 4, ptr %6) #4
  br label %44

22:                                               ; preds = %17
  %23 = load i32, ptr %4, align 4, !tbaa !4
  %24 = load i32, ptr %6, align 4, !tbaa !4
  %25 = add nsw i32 %23, %24
  %26 = sitofp i32 %25 to x86_fp80
  %27 = fmul x86_fp80 %26, 0xK3FFE8000000000000000
  %28 = load i32, ptr %4, align 4, !tbaa !4
  %29 = getelementptr inbounds [16 x [16 x x86_fp80]], ptr @A, i32 0, i32 %28
  %30 = load i32, ptr %6, align 4, !tbaa !4
  %31 = getelementptr inbounds [16 x x86_fp80], ptr %29, i32 0, i32 %30
  store x86_fp80 %27, ptr %31, align 4, !tbaa !8
  %32 = load i32, ptr %4, align 4, !tbaa !4
  %33 = load i32, ptr %6, align 4, !tbaa !4
  %34 = sub nsw i32 %32, %33
  %35 = sitofp i32 %34 to x86_fp80
  %36 = call x86_fp80 @llvm.fmuladd.f80(x86_fp80 %35, x86_fp80 0xK3FFD8000000000000000, x86_fp80 0xK3FFF8000000000000000)
  %37 = load i32, ptr %4, align 4, !tbaa !4
  %38 = getelementptr inbounds [16 x [16 x x86_fp80]], ptr @B, i32 0, i32 %37
  %39 = load i32, ptr %6, align 4, !tbaa !4
  %40 = getelementptr inbounds [16 x x86_fp80], ptr %38, i32 0, i32 %39
  store x86_fp80 %36, ptr %40, align 4, !tbaa !8
  br label %41

41:                                               ; preds = %22
  %42 = load i32, ptr %6, align 4, !tbaa !4
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %6, align 4, !tbaa !4
  br label %17, !llvm.loop !10

44:                                               ; preds = %21
  br label %45

45:                                               ; preds = %44
  %46 = load i32, ptr %4, align 4, !tbaa !4
  %47 = add nsw i32 %46, 1
  store i32 %47, ptr %4, align 4, !tbaa !4
  br label %11, !llvm.loop !12

48:                                               ; preds = %15
  call void @llvm.lifetime.start.p0(i64 4, ptr %7) #4
  store i32 0, ptr %7, align 4, !tbaa !4
  br label %49

49:                                               ; preds = %95, %48
  %50 = load i32, ptr %7, align 4, !tbaa !4
  %51 = load i32, ptr %2, align 4, !tbaa !4
  %52 = icmp slt i32 %50, %51
  br i1 %52, label %54, label %53

53:                                               ; preds = %49
  store i32 8, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 4, ptr %7) #4
  br label %98

54:                                               ; preds = %49
  call void @llvm.lifetime.start.p0(i64 4, ptr %8) #4
  store i32 0, ptr %8, align 4, !tbaa !4
  br label %55

55:                                               ; preds = %91, %54
  %56 = load i32, ptr %8, align 4, !tbaa !4
  %57 = load i32, ptr %2, align 4, !tbaa !4
  %58 = icmp slt i32 %56, %57
  br i1 %58, label %60, label %59

59:                                               ; preds = %55
  store i32 11, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 4, ptr %8) #4
  br label %94

60:                                               ; preds = %55
  call void @llvm.lifetime.start.p0(i64 12, ptr %9) #4
  store x86_fp80 0xK00000000000000000000, ptr %9, align 4, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 4, ptr %10) #4
  store i32 0, ptr %10, align 4, !tbaa !4
  br label %61

61:                                               ; preds = %79, %60
  %62 = load i32, ptr %10, align 4, !tbaa !4
  %63 = load i32, ptr %2, align 4, !tbaa !4
  %64 = icmp slt i32 %62, %63
  br i1 %64, label %66, label %65

65:                                               ; preds = %61
  store i32 14, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 4, ptr %10) #4
  br label %82

66:                                               ; preds = %61
  %67 = load i32, ptr %7, align 4, !tbaa !4
  %68 = getelementptr inbounds [16 x [16 x x86_fp80]], ptr @A, i32 0, i32 %67
  %69 = load i32, ptr %10, align 4, !tbaa !4
  %70 = getelementptr inbounds [16 x x86_fp80], ptr %68, i32 0, i32 %69
  %71 = load x86_fp80, ptr %70, align 4, !tbaa !8
  %72 = load i32, ptr %10, align 4, !tbaa !4
  %73 = getelementptr inbounds [16 x [16 x x86_fp80]], ptr @B, i32 0, i32 %72
  %74 = load i32, ptr %8, align 4, !tbaa !4
  %75 = getelementptr inbounds [16 x x86_fp80], ptr %73, i32 0, i32 %74
  %76 = load x86_fp80, ptr %75, align 4, !tbaa !8
  %77 = load x86_fp80, ptr %9, align 4, !tbaa !8
  %78 = call x86_fp80 @llvm.fmuladd.f80(x86_fp80 %71, x86_fp80 %76, x86_fp80 %77)
  store x86_fp80 %78, ptr %9, align 4, !tbaa !8
  br label %79

79:                                               ; preds = %66
  %80 = load i32, ptr %10, align 4, !tbaa !4
  %81 = add nsw i32 %80, 1
  store i32 %81, ptr %10, align 4, !tbaa !4
  br label %61, !llvm.loop !13

82:                                               ; preds = %65
  %83 = load x86_fp80, ptr %9, align 4, !tbaa !8
  %84 = load i32, ptr %7, align 4, !tbaa !4
  %85 = getelementptr inbounds [16 x [16 x x86_fp80]], ptr @C, i32 0, i32 %84
  %86 = load i32, ptr %8, align 4, !tbaa !4
  %87 = getelementptr inbounds [16 x x86_fp80], ptr %85, i32 0, i32 %86
  store x86_fp80 %83, ptr %87, align 4, !tbaa !8
  %88 = load x86_fp80, ptr %9, align 4, !tbaa !8
  %89 = load x86_fp80, ptr %3, align 4, !tbaa !8
  %90 = fadd x86_fp80 %89, %88
  store x86_fp80 %90, ptr %3, align 4, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 12, ptr %9) #4
  br label %91

91:                                               ; preds = %82
  %92 = load i32, ptr %8, align 4, !tbaa !4
  %93 = add nsw i32 %92, 1
  store i32 %93, ptr %8, align 4, !tbaa !4
  br label %55, !llvm.loop !14

94:                                               ; preds = %59
  br label %95

95:                                               ; preds = %94
  %96 = load i32, ptr %7, align 4, !tbaa !4
  %97 = add nsw i32 %96, 1
  store i32 %97, ptr %7, align 4, !tbaa !4
  br label %49, !llvm.loop !15

98:                                               ; preds = %53
  %99 = load x86_fp80, ptr %3, align 4, !tbaa !8
  store i32 1, ptr %5, align 4
  call void @llvm.lifetime.end.p0(i64 12, ptr %3) #4
  ret x86_fp80 %99
}

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  call void @setup()
  %2 = load volatile ptr, ptr @bench_x_ld_matmul_via, align 4, !tbaa !16
  %3 = call i32 %2(i32 noundef 12)
  call void @report(i32 noundef %3)
  ret i32 0
}

; Function Attrs: nounwind uwtable
define internal void @setup() #0 {
  ret void
}

declare dso_local void @report(i32 noundef) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare x86_fp80 @llvm.fmuladd.f80(x86_fp80, x86_fp80, x86_fp80) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #2

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 4}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!4 = !{!5, !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"long double", !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = distinct !{!14, !11}
!15 = distinct !{!15, !11}
!16 = !{!17, !17, i64 0}
!17 = !{!"any pointer", !6, i64 0}
