target triple = "msp430"

@values = internal constant [4 x i64] [i64 81985529216486895, i64 -1, i64 -9223372036854775807, i64 4294901761], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  store i16 0, ptr %6, align 2, !tbaa !9
  br label %9

9:                                                ; preds = %42, %0
  %10 = load i16, ptr %6, align 2, !tbaa !9
  %11 = icmp slt i16 %10, 4
  br i1 %11, label %12, label %45

12:                                               ; preds = %9
  store i16 0, ptr %7, align 2, !tbaa !9
  br label %13

13:                                               ; preds = %38, %12
  %14 = load i16, ptr %7, align 2, !tbaa !9
  %15 = icmp ult i16 %14, 64
  br i1 %15, label %16, label %41

16:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %17 = load i16, ptr %6, align 2, !tbaa !9
  %18 = getelementptr inbounds [4 x i64], ptr @values, i16 0, i16 %17
  %19 = load i64, ptr %18, align 2, !tbaa !10
  store i64 %19, ptr %8, align 2, !tbaa !10
  %20 = load i32, ptr %2, align 2, !tbaa !7
  %21 = load i64, ptr %8, align 2, !tbaa !10
  %22 = load i16, ptr %7, align 2, !tbaa !9
  %23 = zext i16 %22 to i64
  %24 = shl i64 %21, %23
  %25 = call i32 @mix(i32 noundef %20, i64 noundef %24) #4
  store i32 %25, ptr %2, align 2, !tbaa !7
  %26 = load i32, ptr %3, align 2, !tbaa !7
  %27 = load i64, ptr %8, align 2, !tbaa !10
  %28 = load i16, ptr %7, align 2, !tbaa !9
  %29 = zext i16 %28 to i64
  %30 = lshr i64 %27, %29
  %31 = call i32 @mix(i32 noundef %26, i64 noundef %30) #4
  store i32 %31, ptr %3, align 2, !tbaa !7
  %32 = load i32, ptr %4, align 2, !tbaa !7
  %33 = load i64, ptr %8, align 2, !tbaa !10
  %34 = load i16, ptr %7, align 2, !tbaa !9
  %35 = zext i16 %34 to i64
  %36 = ashr i64 %33, %35
  %37 = call i32 @mix(i32 noundef %32, i64 noundef %36) #4
  store i32 %37, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  br label %38

38:                                               ; preds = %16
  %39 = load i16, ptr %7, align 2, !tbaa !9
  %40 = add i16 %39, 1
  store i16 %40, ptr %7, align 2, !tbaa !9
  br label %13, !llvm.loop !12

41:                                               ; preds = %13
  br label %42

42:                                               ; preds = %41
  %43 = load i16, ptr %6, align 2, !tbaa !9
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %6, align 2, !tbaa !9
  br label %9, !llvm.loop !14

45:                                               ; preds = %9
  store i16 0, ptr %6, align 2, !tbaa !9
  br label %46

46:                                               ; preds = %66, %45
  %47 = load i16, ptr %6, align 2, !tbaa !9
  %48 = icmp slt i16 %47, 4
  br i1 %48, label %49, label %69

49:                                               ; preds = %46
  %50 = load i32, ptr %5, align 2, !tbaa !7
  %51 = load i16, ptr %6, align 2, !tbaa !9
  %52 = getelementptr inbounds [4 x i64], ptr @values, i16 0, i16 %51
  %53 = load i64, ptr %52, align 2, !tbaa !10
  %54 = lshr i64 %53, 30
  %55 = load i16, ptr %6, align 2, !tbaa !9
  %56 = getelementptr inbounds [4 x i64], ptr @values, i16 0, i16 %55
  %57 = load i64, ptr %56, align 2, !tbaa !10
  %58 = shl i64 %57, 33
  %59 = add i64 %54, %58
  %60 = load i16, ptr %6, align 2, !tbaa !9
  %61 = getelementptr inbounds [4 x i64], ptr @values, i16 0, i16 %60
  %62 = load i64, ptr %61, align 2, !tbaa !10
  %63 = ashr i64 %62, 35
  %64 = add i64 %59, %63
  %65 = call i32 @mix(i32 noundef %50, i64 noundef %64) #4
  store i32 %65, ptr %5, align 2, !tbaa !7
  br label %66

66:                                               ; preds = %49
  %67 = load i16, ptr %6, align 2, !tbaa !9
  %68 = add nsw i16 %67, 1
  store i16 %68, ptr %6, align 2, !tbaa !9
  br label %46, !llvm.loop !15

69:                                               ; preds = %46
  %70 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %70) #4
  %71 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %71) #4
  %72 = load i32, ptr %4, align 2, !tbaa !7
  call void @report(i32 noundef %72) #4
  %73 = load i32, ptr %5, align 2, !tbaa !7
  call void @report(i32 noundef %73) #4
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @mix(i32 noundef %0, i64 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i64, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i64 %1, ptr %4, align 2, !tbaa !10
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = mul i32 %5, 31
  %7 = load i64, ptr %4, align 2, !tbaa !10
  %8 = trunc i64 %7 to i32
  %9 = add i32 %6, %8
  %10 = load i64, ptr %4, align 2, !tbaa !10
  %11 = lshr i64 %10, 32
  %12 = trunc i64 %11 to i32
  %13 = mul i32 %12, 7
  %14 = add i32 %9, %13
  ret i32 %14
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long long", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
