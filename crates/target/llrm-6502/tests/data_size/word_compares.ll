target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@v = internal global [14 x i32] [i32 0, i32 1, i32 2, i32 32767, i32 32768, i32 65535, i32 65536, i32 65537, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -2, i32 -1], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %28, %0
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 14
  br i1 %7, label %8, label %31

8:                                                ; preds = %5
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !8
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %23, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 14
  br i1 %11, label %12, label %26

12:                                               ; preds = %9
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = mul i32 %13, 257
  %15 = load i16, ptr %2, align 2, !tbaa !7
  %16 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %15
  %17 = load volatile i32, ptr %16, align 2, !tbaa !8
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %18
  %20 = load volatile i32, ptr %19, align 2, !tbaa !8
  %21 = call i32 @orders(i32 noundef %17, i32 noundef %20) #4
  %22 = add i32 %14, %21
  store i32 %22, ptr %4, align 2, !tbaa !8
  br label %23

23:                                               ; preds = %12
  %24 = load i16, ptr %3, align 2, !tbaa !7
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !10

26:                                               ; preds = %9
  %27 = load i32, ptr %4, align 2, !tbaa !8
  call void @report(i32 noundef %27) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  br label %28

28:                                               ; preds = %26
  %29 = load i16, ptr %2, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %2, align 2, !tbaa !7
  br label %5, !llvm.loop !12

31:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @orders(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %7 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %7, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %8, ptr %6, align 2, !tbaa !8
  %9 = load i32, ptr %3, align 2, !tbaa !8
  %10 = load i32, ptr %4, align 2, !tbaa !8
  %11 = icmp ult i32 %9, %10
  %12 = zext i1 %11 to i16
  %13 = load i32, ptr %3, align 2, !tbaa !8
  %14 = load i32, ptr %4, align 2, !tbaa !8
  %15 = icmp ule i32 %13, %14
  %16 = zext i1 %15 to i16
  %17 = mul nsw i16 2, %16
  %18 = add nsw i16 %12, %17
  %19 = load i32, ptr %3, align 2, !tbaa !8
  %20 = load i32, ptr %4, align 2, !tbaa !8
  %21 = icmp ugt i32 %19, %20
  %22 = zext i1 %21 to i16
  %23 = mul nsw i16 4, %22
  %24 = add nsw i16 %18, %23
  %25 = load i32, ptr %3, align 2, !tbaa !8
  %26 = load i32, ptr %4, align 2, !tbaa !8
  %27 = icmp uge i32 %25, %26
  %28 = zext i1 %27 to i16
  %29 = mul nsw i16 8, %28
  %30 = add nsw i16 %24, %29
  %31 = load i32, ptr %5, align 2, !tbaa !8
  %32 = load i32, ptr %6, align 2, !tbaa !8
  %33 = icmp slt i32 %31, %32
  %34 = zext i1 %33 to i16
  %35 = mul nsw i16 16, %34
  %36 = add nsw i16 %30, %35
  %37 = load i32, ptr %5, align 2, !tbaa !8
  %38 = load i32, ptr %6, align 2, !tbaa !8
  %39 = icmp sle i32 %37, %38
  %40 = zext i1 %39 to i16
  %41 = mul nsw i16 32, %40
  %42 = add nsw i16 %36, %41
  %43 = load i32, ptr %5, align 2, !tbaa !8
  %44 = load i32, ptr %6, align 2, !tbaa !8
  %45 = icmp sgt i32 %43, %44
  %46 = zext i1 %45 to i16
  %47 = mul nsw i16 64, %46
  %48 = add nsw i16 %42, %47
  %49 = load i32, ptr %5, align 2, !tbaa !8
  %50 = load i32, ptr %6, align 2, !tbaa !8
  %51 = icmp sge i32 %49, %50
  %52 = zext i1 %51 to i16
  %53 = mul nsw i16 128, %52
  %54 = add nsw i16 %48, %53
  %55 = sext i16 %54 to i32
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %55
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
