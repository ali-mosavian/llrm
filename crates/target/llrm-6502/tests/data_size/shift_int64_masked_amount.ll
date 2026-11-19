target triple = "msp430"

@values = internal constant [4 x i64] [i64 81985529216486895, i64 -1, i64 -9223372036854775807, i64 4294901761], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i64, align 2
  %8 = alloca i64, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %9

9:                                                ; preds = %44, %0
  %10 = load i16, ptr %5, align 2, !tbaa !9
  %11 = icmp slt i16 %10, 4
  br i1 %11, label %12, label %47

12:                                               ; preds = %9
  store i16 0, ptr %6, align 2, !tbaa !9
  br label %13

13:                                               ; preds = %40, %12
  %14 = load i16, ptr %6, align 2, !tbaa !9
  %15 = icmp ult i16 %14, 64
  br i1 %15, label %16, label %43

16:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %17 = load i16, ptr %5, align 2, !tbaa !9
  %18 = getelementptr inbounds [4 x i64], ptr @values, i16 0, i16 %17
  %19 = load i64, ptr %18, align 2, !tbaa !10
  store i64 %19, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #3
  %20 = load i16, ptr %6, align 2, !tbaa !9
  %21 = zext i16 %20 to i64
  store i64 %21, ptr %8, align 2, !tbaa !10
  %22 = load i32, ptr %2, align 2, !tbaa !7
  %23 = load i64, ptr %7, align 2, !tbaa !10
  %24 = load i64, ptr %8, align 2, !tbaa !10
  %25 = and i64 %24, 31
  %26 = shl i64 %23, %25
  %27 = call i32 @mix(i32 noundef %22, i64 noundef %26) #4
  store i32 %27, ptr %2, align 2, !tbaa !7
  %28 = load i32, ptr %3, align 2, !tbaa !7
  %29 = load i64, ptr %7, align 2, !tbaa !10
  %30 = load i64, ptr %8, align 2, !tbaa !10
  %31 = and i64 %30, 31
  %32 = lshr i64 %29, %31
  %33 = call i32 @mix(i32 noundef %28, i64 noundef %32) #4
  store i32 %33, ptr %3, align 2, !tbaa !7
  %34 = load i32, ptr %4, align 2, !tbaa !7
  %35 = load i64, ptr %7, align 2, !tbaa !10
  %36 = load i64, ptr %8, align 2, !tbaa !10
  %37 = and i64 %36, 31
  %38 = ashr i64 %35, %37
  %39 = call i32 @mix(i32 noundef %34, i64 noundef %38) #4
  store i32 %39, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %40

40:                                               ; preds = %16
  %41 = load i16, ptr %6, align 2, !tbaa !9
  %42 = add i16 %41, 1
  store i16 %42, ptr %6, align 2, !tbaa !9
  br label %13, !llvm.loop !12

43:                                               ; preds = %13
  br label %44

44:                                               ; preds = %43
  %45 = load i16, ptr %5, align 2, !tbaa !9
  %46 = add nsw i16 %45, 1
  store i16 %46, ptr %5, align 2, !tbaa !9
  br label %9, !llvm.loop !14

47:                                               ; preds = %9
  %48 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %48) #4
  %49 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %49) #4
  %50 = load i32, ptr %4, align 2, !tbaa !7
  call void @report(i32 noundef %50) #4
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
