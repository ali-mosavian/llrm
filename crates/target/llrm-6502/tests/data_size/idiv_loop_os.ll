target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i32 @total(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 1, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %24, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp sle i16 %6, %7
  br i1 %8, label %9, label %27

9:                                                ; preds = %5
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = mul nsw i16 %10, 7
  %12 = add nsw i16 %11, 3
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = mul nsw i16 %13, 5
  %15 = add nsw i16 %14, 2
  %16 = call i16 @gcd(i16 noundef %12, i16 noundef %15) #4
  %17 = load i16, ptr %4, align 2, !tbaa !7
  %18 = add nsw i16 %17, 90
  %19 = call i16 @gcd(i16 noundef %18, i16 noundef 360) #4
  %20 = add nsw i16 %16, %19
  %21 = sext i16 %20 to i32
  %22 = load i32, ptr %3, align 2, !tbaa !8
  %23 = add nsw i32 %22, %21
  store i32 %23, ptr %3, align 2, !tbaa !8
  br label %24

24:                                               ; preds = %9
  %25 = load i16, ptr %4, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !10

27:                                               ; preds = %5
  %28 = load i32, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %28
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i16 @gcd(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %5, align 2, !tbaa !7
  %7 = icmp eq i16 %6, 0
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i16, ptr %4, align 2, !tbaa !7
  store i16 %9, ptr %3, align 2
  br label %16

10:                                               ; preds = %2
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = load i16, ptr %4, align 2, !tbaa !7
  %13 = load i16, ptr %5, align 2, !tbaa !7
  %14 = srem i16 %12, %13
  %15 = call i16 @gcd(i16 noundef %11, i16 noundef %14) #4
  store i16 %15, ptr %3, align 2
  br label %16

16:                                               ; preds = %10, %8
  %17 = load i16, ptr %3, align 2
  ret i16 %17
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @total(i16 noundef 300) #4
  call void @report(i32 noundef %2) #4
  ret i16 0
}

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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
