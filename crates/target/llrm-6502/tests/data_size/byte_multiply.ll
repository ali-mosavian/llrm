target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
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
  br label %7

7:                                                ; preds = %35, %0
  %8 = load i16, ptr %5, align 2, !tbaa !9
  %9 = icmp slt i16 %8, 256
  br i1 %9, label %10, label %38

10:                                               ; preds = %7
  store i16 0, ptr %6, align 2, !tbaa !9
  br label %11

11:                                               ; preds = %31, %10
  %12 = load i16, ptr %6, align 2, !tbaa !9
  %13 = icmp slt i16 %12, 256
  br i1 %13, label %14, label %34

14:                                               ; preds = %11
  %15 = load i16, ptr %5, align 2, !tbaa !9
  %16 = trunc i16 %15 to i8
  %17 = load i16, ptr %6, align 2, !tbaa !9
  %18 = trunc i16 %17 to i8
  %19 = call zeroext i8 @unsigned_product(i8 noundef zeroext %16, i8 noundef zeroext %18) #4
  %20 = zext i8 %19 to i32
  %21 = load i32, ptr %2, align 2, !tbaa !7
  %22 = add nsw i32 %21, %20
  store i32 %22, ptr %2, align 2, !tbaa !7
  %23 = load i16, ptr %5, align 2, !tbaa !9
  %24 = trunc i16 %23 to i8
  %25 = load i16, ptr %6, align 2, !tbaa !9
  %26 = trunc i16 %25 to i8
  %27 = call signext i8 @signed_product(i8 noundef signext %24, i8 noundef signext %26) #4
  %28 = sext i8 %27 to i32
  %29 = load i32, ptr %3, align 2, !tbaa !7
  %30 = add nsw i32 %29, %28
  store i32 %30, ptr %3, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %14
  %32 = load i16, ptr %6, align 2, !tbaa !9
  %33 = add nsw i16 %32, 7
  store i16 %33, ptr %6, align 2, !tbaa !9
  br label %11, !llvm.loop !10

34:                                               ; preds = %11
  br label %35

35:                                               ; preds = %34
  %36 = load i16, ptr %5, align 2, !tbaa !9
  %37 = add nsw i16 %36, 5
  store i16 %37, ptr %5, align 2, !tbaa !9
  br label %7, !llvm.loop !12

38:                                               ; preds = %7
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %39

39:                                               ; preds = %55, %38
  %40 = load i16, ptr %5, align 2, !tbaa !9
  %41 = icmp slt i16 %40, 256
  br i1 %41, label %42, label %58

42:                                               ; preds = %39
  %43 = load i16, ptr %5, align 2, !tbaa !9
  %44 = trunc i16 %43 to i8
  %45 = call zeroext i8 @by_constant(i8 noundef zeroext %44) #4
  %46 = zext i8 %45 to i32
  %47 = mul nsw i32 %46, 3
  %48 = load i16, ptr %5, align 2, !tbaa !9
  %49 = trunc i16 %48 to i8
  %50 = call signext i8 @by_negative(i8 noundef signext %49) #4
  %51 = sext i8 %50 to i32
  %52 = add nsw i32 %47, %51
  %53 = load i32, ptr %4, align 2, !tbaa !7
  %54 = add nsw i32 %53, %52
  store i32 %54, ptr %4, align 2, !tbaa !7
  br label %55

55:                                               ; preds = %42
  %56 = load i16, ptr %5, align 2, !tbaa !9
  %57 = add nsw i16 %56, 1
  store i16 %57, ptr %5, align 2, !tbaa !9
  br label %39, !llvm.loop !13

58:                                               ; preds = %39
  %59 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %59) #4
  %60 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %60) #4
  %61 = load i32, ptr %4, align 2, !tbaa !7
  call void @report(i32 noundef %61) #4
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
define internal zeroext i8 @unsigned_product(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !14
  store i8 %1, ptr %4, align 1, !tbaa !14
  %5 = load i8, ptr %3, align 1, !tbaa !14
  %6 = zext i8 %5 to i16
  %7 = load i8, ptr %4, align 1, !tbaa !14
  %8 = zext i8 %7 to i16
  %9 = mul nsw i16 %6, %8
  %10 = trunc i16 %9 to i8
  ret i8 %10
}

; Function Attrs: nounwind optsize
define internal signext i8 @signed_product(i8 noundef signext %0, i8 noundef signext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !14
  store i8 %1, ptr %4, align 1, !tbaa !14
  %5 = load i8, ptr %3, align 1, !tbaa !14
  %6 = sext i8 %5 to i16
  %7 = load i8, ptr %4, align 1, !tbaa !14
  %8 = sext i8 %7 to i16
  %9 = mul nsw i16 %6, %8
  %10 = trunc i16 %9 to i8
  ret i8 %10
}

; Function Attrs: nounwind optsize
define internal zeroext i8 @by_constant(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !14
  %3 = load i8, ptr %2, align 1, !tbaa !14
  %4 = zext i8 %3 to i16
  %5 = mul nsw i16 %4, 200
  %6 = trunc i16 %5 to i8
  ret i8 %6
}

; Function Attrs: nounwind optsize
define internal signext i8 @by_negative(i8 noundef signext %0) #0 {
  %2 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !14
  %3 = load i8, ptr %2, align 1, !tbaa !14
  %4 = sext i8 %3 to i16
  %5 = mul nsw i16 %4, -7
  %6 = trunc i16 %5 to i8
  ret i8 %6
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

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
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = !{!5, !5, i64 0}
