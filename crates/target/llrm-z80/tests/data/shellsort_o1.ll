target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'shellsort.c'
source_filename = "shellsort.c"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i32 @bench_shellsort(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [64 x i16], align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i32 0, ptr %6, align 2, !tbaa !9
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %27, %1
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = icmp ult i16 %10, 64
  br i1 %11, label %12, label %30

12:                                               ; preds = %9
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = zext i16 %13 to i32
  %15 = mul i32 %14, 109
  %16 = add i32 %15, 37
  %17 = load i16, ptr %4, align 2, !tbaa !7
  %18 = shl i16 %17, 7
  %19 = zext i16 %18 to i32
  %20 = xor i32 %16, %19
  %21 = load i16, ptr %2, align 2, !tbaa !7
  %22 = zext i16 %21 to i32
  %23 = xor i32 %20, %22
  %24 = trunc i32 %23 to i16
  %25 = load i16, ptr %4, align 2, !tbaa !7
  %26 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %25
  store i16 %24, ptr %26, align 2, !tbaa !7
  br label %27

27:                                               ; preds = %12
  %28 = load i16, ptr %4, align 2, !tbaa !7
  %29 = add i16 %28, 1
  store i16 %29, ptr %4, align 2, !tbaa !7
  br label %9, !llvm.loop !11

30:                                               ; preds = %9
  store i16 32, ptr %5, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %77, %30
  %32 = load i16, ptr %5, align 2, !tbaa !7
  %33 = icmp ne i16 %32, 0
  br i1 %33, label %34, label %80

34:                                               ; preds = %31
  %35 = load i16, ptr %5, align 2, !tbaa !7
  store i16 %35, ptr %4, align 2, !tbaa !7
  br label %36

36:                                               ; preds = %73, %34
  %37 = load i16, ptr %4, align 2, !tbaa !7
  %38 = icmp ult i16 %37, 64
  br i1 %38, label %39, label %76

39:                                               ; preds = %36
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %40 = load i16, ptr %4, align 2, !tbaa !7
  store i16 %40, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %41 = load i16, ptr %4, align 2, !tbaa !7
  %42 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %41
  %43 = load i16, ptr %42, align 2, !tbaa !7
  store i16 %43, ptr %8, align 2, !tbaa !7
  br label %44

44:                                               ; preds = %58, %39
  %45 = load i16, ptr %7, align 2, !tbaa !7
  %46 = load i16, ptr %5, align 2, !tbaa !7
  %47 = icmp uge i16 %45, %46
  br i1 %47, label %48, label %56

48:                                               ; preds = %44
  %49 = load i16, ptr %7, align 2, !tbaa !7
  %50 = load i16, ptr %5, align 2, !tbaa !7
  %51 = sub i16 %49, %50
  %52 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %51
  %53 = load i16, ptr %52, align 2, !tbaa !7
  %54 = load i16, ptr %8, align 2, !tbaa !7
  %55 = icmp ugt i16 %53, %54
  br label %56

56:                                               ; preds = %48, %44
  %57 = phi i1 [ false, %44 ], [ %55, %48 ]
  br i1 %57, label %58, label %69

58:                                               ; preds = %56
  %59 = load i16, ptr %7, align 2, !tbaa !7
  %60 = load i16, ptr %5, align 2, !tbaa !7
  %61 = sub i16 %59, %60
  %62 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %61
  %63 = load i16, ptr %62, align 2, !tbaa !7
  %64 = load i16, ptr %7, align 2, !tbaa !7
  %65 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %64
  store i16 %63, ptr %65, align 2, !tbaa !7
  %66 = load i16, ptr %7, align 2, !tbaa !7
  %67 = load i16, ptr %5, align 2, !tbaa !7
  %68 = sub i16 %66, %67
  store i16 %68, ptr %7, align 2, !tbaa !7
  br label %44, !llvm.loop !14

69:                                               ; preds = %56
  %70 = load i16, ptr %8, align 2, !tbaa !7
  %71 = load i16, ptr %7, align 2, !tbaa !7
  %72 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %71
  store i16 %70, ptr %72, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %73

73:                                               ; preds = %69
  %74 = load i16, ptr %4, align 2, !tbaa !7
  %75 = add i16 %74, 1
  store i16 %75, ptr %4, align 2, !tbaa !7
  br label %36, !llvm.loop !15

76:                                               ; preds = %36
  br label %77

77:                                               ; preds = %76
  %78 = load i16, ptr %5, align 2, !tbaa !7
  %79 = lshr i16 %78, 1
  store i16 %79, ptr %5, align 2, !tbaa !7
  br label %31, !llvm.loop !16

80:                                               ; preds = %31
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %81

81:                                               ; preds = %95, %80
  %82 = load i16, ptr %4, align 2, !tbaa !7
  %83 = icmp ult i16 %82, 64
  br i1 %83, label %84, label %98

84:                                               ; preds = %81
  %85 = load i16, ptr %4, align 2, !tbaa !7
  %86 = getelementptr inbounds nuw [64 x i16], ptr %3, i16 0, i16 %85
  %87 = load i16, ptr %86, align 2, !tbaa !7
  %88 = zext i16 %87 to i32
  %89 = load i16, ptr %4, align 2, !tbaa !7
  %90 = add i16 %89, 1
  %91 = zext i16 %90 to i32
  %92 = mul i32 %88, %91
  %93 = load i32, ptr %6, align 2, !tbaa !9
  %94 = add i32 %93, %92
  store i32 %94, ptr %6, align 2, !tbaa !9
  br label %95

95:                                               ; preds = %84
  %96 = load i16, ptr %4, align 2, !tbaa !7
  %97 = add i16 %96, 1
  store i16 %97, ptr %4, align 2, !tbaa !7
  br label %81, !llvm.loop !17

98:                                               ; preds = %81
  %99 = load i32, ptr %6, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %99
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_shellsort(i16 noundef zeroext 0)
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
!16 = distinct !{!16, !12, !13}
!17 = distinct !{!17, !12, !13}
