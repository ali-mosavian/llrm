target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@table = internal global ptr @store, align 2
@store = internal global [903 x i16] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i16, align 2
  %4 = alloca ptr, align 2
  %5 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 1, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %6 = load volatile ptr, ptr @table, align 2, !tbaa !9
  store ptr %6, ptr %4, align 2, !tbaa !9
  store i16 1, ptr %3, align 2, !tbaa !12
  br label %7

7:                                                ; preds = %27, %0
  %8 = load i16, ptr %3, align 2, !tbaa !12
  %9 = icmp sle i16 %8, 300
  br i1 %9, label %10, label %30

10:                                               ; preds = %7
  %11 = load i32, ptr %2, align 2, !tbaa !7
  %12 = mul nsw i32 %11, 25173
  %13 = add nsw i32 %12, 13849
  %14 = and i32 %13, 65535
  store i32 %14, ptr %2, align 2, !tbaa !7
  %15 = load i32, ptr %2, align 2, !tbaa !7
  %16 = and i32 %15, 4095
  %17 = trunc i32 %16 to i16
  %18 = load ptr, ptr %4, align 2, !tbaa !9
  %19 = load i16, ptr %3, align 2, !tbaa !12
  %20 = mul nsw i16 3, %19
  %21 = getelementptr inbounds i16, ptr %18, i16 %20
  store i16 %17, ptr %21, align 2, !tbaa !12
  %22 = load i16, ptr %3, align 2, !tbaa !12
  %23 = icmp sgt i16 %22, 1
  br i1 %23, label %24, label %26

24:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 1, ptr %5, align 2, !tbaa !12
  %25 = load i16, ptr %3, align 2, !tbaa !12
  call void @add(ptr noundef %5, i16 noundef signext %25) #4
  call void @llvm.lifetime.end.p0(ptr %5) #3
  br label %26

26:                                               ; preds = %24, %10
  br label %27

27:                                               ; preds = %26
  %28 = load i16, ptr %3, align 2, !tbaa !12
  %29 = add nsw i16 %28, 1
  store i16 %29, ptr %3, align 2, !tbaa !12
  br label %7, !llvm.loop !14

30:                                               ; preds = %7
  %31 = call i32 @walk(i16 noundef signext 1, i32 noundef 1) #4
  call void @report(i32 noundef %31) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @add(ptr noundef %0, i16 noundef signext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca ptr, align 2
  %8 = alloca i32, align 2
  store ptr %0, ptr %3, align 2, !tbaa !9
  store i16 %1, ptr %4, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %9 = load volatile ptr, ptr @table, align 2, !tbaa !9
  store ptr %9, ptr %5, align 2, !tbaa !9
  br label %10

10:                                               ; preds = %42, %2
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %11 = load ptr, ptr %5, align 2, !tbaa !9
  %12 = load i16, ptr %4, align 2, !tbaa !12
  %13 = mul nsw i16 3, %12
  %14 = getelementptr inbounds i16, ptr %11, i16 %13
  %15 = load i16, ptr %14, align 2, !tbaa !12
  %16 = load ptr, ptr %5, align 2, !tbaa !9
  %17 = load ptr, ptr %3, align 2, !tbaa !9
  %18 = load i16, ptr %17, align 2, !tbaa !12
  %19 = mul nsw i16 3, %18
  %20 = getelementptr inbounds i16, ptr %16, i16 %19
  %21 = load i16, ptr %20, align 2, !tbaa !12
  %22 = icmp slt i16 %15, %21
  %23 = zext i1 %22 to i64
  %24 = select i1 %22, i16 1, i16 2
  store i16 %24, ptr %6, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %25 = load ptr, ptr %5, align 2, !tbaa !9
  %26 = load ptr, ptr %3, align 2, !tbaa !9
  %27 = load i16, ptr %26, align 2, !tbaa !12
  %28 = mul nsw i16 3, %27
  %29 = load i16, ptr %6, align 2, !tbaa !12
  %30 = add nsw i16 %28, %29
  %31 = getelementptr inbounds i16, ptr %25, i16 %30
  store ptr %31, ptr %7, align 2, !tbaa !9
  %32 = load ptr, ptr %7, align 2, !tbaa !9
  %33 = load i16, ptr %32, align 2, !tbaa !12
  %34 = icmp eq i16 %33, 0
  br i1 %34, label %35, label %38

35:                                               ; preds = %10
  %36 = load i16, ptr %4, align 2, !tbaa !12
  %37 = load ptr, ptr %7, align 2, !tbaa !9
  store i16 %36, ptr %37, align 2, !tbaa !12
  store i32 1, ptr %8, align 2
  br label %40

38:                                               ; preds = %10
  %39 = load ptr, ptr %7, align 2, !tbaa !9
  store ptr %39, ptr %3, align 2, !tbaa !9
  store i32 0, ptr %8, align 2
  br label %40

40:                                               ; preds = %38, %35
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %41 = load i32, ptr %8, align 2
  switch i32 %41, label %43 [
    i32 0, label %42
  ]

42:                                               ; preds = %40
  br label %10

43:                                               ; preds = %40
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @walk(i16 noundef signext %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca ptr, align 2
  %7 = alloca i32, align 2
  store i16 %0, ptr %4, align 2, !tbaa !12
  store i32 %1, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load volatile ptr, ptr @table, align 2, !tbaa !9
  store ptr %8, ptr %6, align 2, !tbaa !9
  %9 = load i16, ptr %4, align 2, !tbaa !12
  %10 = icmp eq i16 %9, 0
  br i1 %10, label %11, label %12

11:                                               ; preds = %2
  store i32 0, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %41

12:                                               ; preds = %2
  %13 = load ptr, ptr %6, align 2, !tbaa !9
  %14 = load i16, ptr %4, align 2, !tbaa !12
  %15 = mul nsw i16 3, %14
  %16 = add nsw i16 %15, 1
  %17 = getelementptr inbounds i16, ptr %13, i16 %16
  %18 = load i16, ptr %17, align 2, !tbaa !12
  %19 = load i32, ptr %5, align 2, !tbaa !7
  %20 = add nsw i32 %19, 1
  %21 = call i32 @walk(i16 noundef signext %18, i32 noundef %20) #4
  %22 = load ptr, ptr %6, align 2, !tbaa !9
  %23 = load i16, ptr %4, align 2, !tbaa !12
  %24 = mul nsw i16 3, %23
  %25 = getelementptr inbounds i16, ptr %22, i16 %24
  %26 = load i16, ptr %25, align 2, !tbaa !12
  %27 = sext i16 %26 to i32
  %28 = load i32, ptr %5, align 2, !tbaa !7
  %29 = mul nsw i32 %27, %28
  %30 = add nsw i32 %21, %29
  %31 = load ptr, ptr %6, align 2, !tbaa !9
  %32 = load i16, ptr %4, align 2, !tbaa !12
  %33 = mul nsw i16 3, %32
  %34 = add nsw i16 %33, 2
  %35 = getelementptr inbounds i16, ptr %31, i16 %34
  %36 = load i16, ptr %35, align 2, !tbaa !12
  %37 = load i32, ptr %5, align 2, !tbaa !7
  %38 = add nsw i32 %37, 1
  %39 = call i32 @walk(i16 noundef signext %36, i32 noundef %38) #4
  %40 = add nsw i32 %30, %39
  store i32 %40, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %41

41:                                               ; preds = %12, %11
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %42 = load i32, ptr %3, align 2
  ret i32 %42
}

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
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 short", !11, i64 0}
!11 = !{!"any pointer", !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"short", !5, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
