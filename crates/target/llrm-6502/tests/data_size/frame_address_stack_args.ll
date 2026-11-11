target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.L = type { ptr, i16, i16, i16, i16, [2 x i16], i16, i16, i16, i16, ptr, i16 }

@seen = internal global i32 0, align 2
@cells = internal global [64 x i8] zeroinitializer, align 1
@pitch = internal global i16 320, align 2

; Function Attrs: nounwind optsize
define dso_local void @terms(ptr noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !10
  store i16 %2, ptr %6, align 2, !tbaa !10
  %7 = load i16, ptr %5, align 2, !tbaa !10
  %8 = load i16, ptr %6, align 2, !tbaa !10
  %9 = add i16 %7, %8
  %10 = load ptr, ptr %4, align 2, !tbaa !7
  %11 = getelementptr inbounds nuw %struct.L, ptr %10, i32 0, i32 1
  store i16 %9, ptr %11, align 1, !tbaa !11
  %12 = load i16, ptr %5, align 2, !tbaa !10
  %13 = load i16, ptr %6, align 2, !tbaa !10
  %14 = sub nsw i16 %12, %13
  %15 = load ptr, ptr %4, align 2, !tbaa !7
  %16 = getelementptr inbounds nuw %struct.L, ptr %15, i32 0, i32 2
  store i16 %14, ptr %16, align 1, !tbaa !14
  %17 = load ptr, ptr %4, align 2, !tbaa !7
  %18 = getelementptr inbounds nuw %struct.L, ptr %17, i32 0, i32 3
  store i16 4, ptr %18, align 1, !tbaa !15
  %19 = load i16, ptr %5, align 2, !tbaa !10
  %20 = zext i16 %19 to i32
  %21 = mul nsw i32 %20, 100
  %22 = load i16, ptr %6, align 2, !tbaa !10
  %23 = zext i16 %22 to i32
  %24 = add nsw i32 %21, %23
  store i32 %24, ptr @seen, align 2, !tbaa !16
  ret void
}

; Function Attrs: nounwind optsize
define dso_local ptr @at(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !10
  store i16 %1, ptr %4, align 2, !tbaa !10
  %5 = load i16, ptr %3, align 2, !tbaa !10
  %6 = load i16, ptr %4, align 2, !tbaa !10
  %7 = add i16 %5, %6
  %8 = urem i16 %7, 64
  %9 = getelementptr inbounds nuw i8, ptr @cells, i16 %8
  ret ptr %9
}

; Function Attrs: nounwind optsize
define dso_local void @kernel(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !7
  %3 = load ptr, ptr %2, align 2, !tbaa !7
  %4 = getelementptr inbounds nuw %struct.L, ptr %3, i32 0, i32 1
  %5 = load i16, ptr %4, align 1, !tbaa !11
  %6 = zext i16 %5 to i32
  %7 = mul nsw i32 %6, 1000
  %8 = load ptr, ptr %2, align 2, !tbaa !7
  %9 = getelementptr inbounds nuw %struct.L, ptr %8, i32 0, i32 5
  %10 = getelementptr inbounds [2 x i16], ptr %9, i16 0, i16 0
  %11 = load i16, ptr %10, align 1, !tbaa !10
  %12 = sext i16 %11 to i32
  %13 = add nsw i32 %7, %12
  %14 = load ptr, ptr %2, align 2, !tbaa !7
  %15 = getelementptr inbounds nuw %struct.L, ptr %14, i32 0, i32 5
  %16 = getelementptr inbounds [2 x i16], ptr %15, i16 0, i16 1
  %17 = load i16, ptr %16, align 1, !tbaa !10
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %13, %18
  %20 = load ptr, ptr %2, align 2, !tbaa !7
  %21 = getelementptr inbounds nuw %struct.L, ptr %20, i32 0, i32 6
  %22 = load i16, ptr %21, align 1, !tbaa !18
  %23 = zext i16 %22 to i32
  %24 = add nsw i32 %19, %23
  %25 = load ptr, ptr %2, align 2, !tbaa !7
  %26 = getelementptr inbounds nuw %struct.L, ptr %25, i32 0, i32 0
  %27 = load ptr, ptr %26, align 1, !tbaa !19
  %28 = call ptr @at(i16 noundef 1, i16 noundef 2) #3
  %29 = icmp eq ptr %27, %28
  %30 = zext i1 %29 to i16
  %31 = sext i16 %30 to i32
  %32 = add nsw i32 %24, %31
  %33 = load i32, ptr @seen, align 2, !tbaa !16
  %34 = add nsw i32 %32, %33
  call void @report(i32 noundef %34) #3
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define dso_local void @line(ptr noundef %0, i16 noundef %1, i16 noundef %2, i16 noundef %3, i16 noundef %4, i16 noundef %5) #0 {
  %7 = alloca ptr, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca i16, align 2
  %13 = alloca %struct.L, align 1
  store ptr %0, ptr %7, align 2, !tbaa !20
  store i16 %1, ptr %8, align 2, !tbaa !10
  store i16 %2, ptr %9, align 2, !tbaa !10
  store i16 %3, ptr %10, align 2, !tbaa !10
  store i16 %4, ptr %11, align 2, !tbaa !10
  store i16 %5, ptr %12, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %14 = load ptr, ptr %7, align 2, !tbaa !20
  %15 = load i16, ptr %10, align 2, !tbaa !10
  %16 = load i16, ptr %11, align 2, !tbaa !10
  call void @terms(ptr noundef %13, i16 noundef %15, i16 noundef %16) #3
  %17 = load i16, ptr %8, align 2, !tbaa !10
  %18 = load i16, ptr %9, align 2, !tbaa !10
  %19 = call ptr @at(i16 noundef %17, i16 noundef %18) #3
  %20 = getelementptr inbounds nuw %struct.L, ptr %13, i32 0, i32 0
  store ptr %19, ptr %20, align 1, !tbaa !19
  %21 = load i16, ptr %12, align 2, !tbaa !10
  %22 = icmp sgt i16 %21, 0
  br i1 %22, label %23, label %25

23:                                               ; preds = %6
  %24 = load i16, ptr @pitch, align 2, !tbaa !10
  br label %28

25:                                               ; preds = %6
  %26 = load i16, ptr @pitch, align 2, !tbaa !10
  %27 = sub nsw i16 0, %26
  br label %28

28:                                               ; preds = %25, %23
  %29 = phi i16 [ %24, %23 ], [ %27, %25 ]
  %30 = getelementptr inbounds nuw %struct.L, ptr %13, i32 0, i32 5
  %31 = getelementptr inbounds [2 x i16], ptr %30, i16 0, i16 1
  store i16 %29, ptr %31, align 1, !tbaa !10
  %32 = getelementptr inbounds nuw %struct.L, ptr %13, i32 0, i32 5
  %33 = getelementptr inbounds [2 x i16], ptr %32, i16 0, i16 0
  store i16 %29, ptr %33, align 1, !tbaa !10
  %34 = getelementptr inbounds nuw %struct.L, ptr %13, i32 0, i32 6
  store i16 0, ptr %34, align 1, !tbaa !18
  call void @kernel(ptr noundef %13) #3
  call void @llvm.lifetime.end.p0(ptr %13) #4
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @line(ptr noundef null, i16 noundef 1, i16 noundef 2, i16 noundef 7, i16 noundef 5, i16 noundef 1) #3
  call void @line(ptr noundef null, i16 noundef 3, i16 noundef 4, i16 noundef 9, i16 noundef 2, i16 noundef -1) #3
  ret i16 0
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
!8 = !{!"p1 _ZTS1L", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!12, !4, i64 2}
!12 = !{!"L", !9, i64 0, !4, i64 2, !4, i64 4, !4, i64 6, !4, i64 8, !5, i64 10, !4, i64 14, !4, i64 16, !4, i64 18, !4, i64 20, !13, i64 22, !4, i64 24}
!13 = !{!"p1 omnipotent char", !9, i64 0}
!14 = !{!12, !4, i64 4}
!15 = !{!12, !4, i64 6}
!16 = !{!17, !17, i64 0}
!17 = !{!"long", !5, i64 0}
!18 = !{!12, !4, i64 14}
!19 = !{!12, !9, i64 0}
!20 = !{!9, !9, i64 0}
