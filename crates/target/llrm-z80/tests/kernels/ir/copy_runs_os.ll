; ModuleID = 'copy_runs.tmp.bc'
source_filename = "/home/alim/scratch/wt-z80ra/tests/run/c/copy_runs.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@s = internal global [48 x i16] zeroinitializer, align 2
@t = internal global [49 x i16] zeroinitializer, align 2
@v = internal global [48 x i8] zeroinitializer, align 1
@d = internal global [48 x i16] zeroinitializer, align 2
@w = internal global [48 x i8] zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %22, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp slt i16 %4, 48
  br i1 %5, label %6, label %25

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = mul nsw i16 %7, 7
  %9 = add nsw i16 %8, 1
  %10 = load i16, ptr %2, align 2, !tbaa !7
  %11 = getelementptr inbounds [48 x i16], ptr @s, i16 0, i16 %10
  store i16 %9, ptr %11, align 2, !tbaa !8
  %12 = load i16, ptr %2, align 2, !tbaa !7
  %13 = mul nsw i16 %12, 5
  %14 = add nsw i16 %13, 2
  %15 = load i16, ptr %2, align 2, !tbaa !7
  %16 = getelementptr inbounds [49 x i16], ptr @t, i16 0, i16 %15
  store i16 %14, ptr %16, align 2, !tbaa !8
  %17 = load i16, ptr %2, align 2, !tbaa !7
  %18 = sub nsw i16 %17, 20
  %19 = trunc i16 %18 to i8
  %20 = load i16, ptr %2, align 2, !tbaa !7
  %21 = getelementptr inbounds [48 x i8], ptr @v, i16 0, i16 %20
  store i8 %19, ptr %21, align 1, !tbaa !10
  br label %22

22:                                               ; preds = %6
  %23 = load i16, ptr %2, align 2, !tbaa !7
  %24 = add nsw i16 %23, 1
  store i16 %24, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !11

25:                                               ; preds = %3
  call void @copy() #4
  call void @smear() #4
  call void @narrow() #4
  %26 = call i32 @sum(ptr noundef @d, i16 noundef 48) #4
  call void @report(i32 noundef %26) #4
  %27 = call i32 @sum(ptr noundef @t, i16 noundef 49) #4
  call void @report(i32 noundef %27) #4
  %28 = load i8, ptr @w, align 1, !tbaa !10
  %29 = zext i8 %28 to i16
  %30 = load i8, ptr getelementptr inbounds nuw (i8, ptr @w, i16 47), align 1, !tbaa !10
  %31 = zext i8 %30 to i16
  %32 = add nsw i16 %29, %31
  %33 = load i8, ptr getelementptr inbounds nuw (i8, ptr @w, i16 17), align 1, !tbaa !10
  %34 = zext i8 %33 to i16
  %35 = add nsw i16 %32, %34
  %36 = sext i16 %35 to i32
  call void @report(i32 noundef %36) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @copy() #0 {
  %1 = load i16, ptr @s, align 2, !tbaa !8
  store i16 %1, ptr @d, align 2, !tbaa !8
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 2), align 2, !tbaa !8
  store i16 %2, ptr getelementptr inbounds nuw (i8, ptr @d, i16 2), align 2, !tbaa !8
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 4), align 2, !tbaa !8
  store i16 %3, ptr getelementptr inbounds nuw (i8, ptr @d, i16 4), align 2, !tbaa !8
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 6), align 2, !tbaa !8
  store i16 %4, ptr getelementptr inbounds nuw (i8, ptr @d, i16 6), align 2, !tbaa !8
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 8), align 2, !tbaa !8
  store i16 %5, ptr getelementptr inbounds nuw (i8, ptr @d, i16 8), align 2, !tbaa !8
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 10), align 2, !tbaa !8
  store i16 %6, ptr getelementptr inbounds nuw (i8, ptr @d, i16 10), align 2, !tbaa !8
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 12), align 2, !tbaa !8
  store i16 %7, ptr getelementptr inbounds nuw (i8, ptr @d, i16 12), align 2, !tbaa !8
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 14), align 2, !tbaa !8
  store i16 %8, ptr getelementptr inbounds nuw (i8, ptr @d, i16 14), align 2, !tbaa !8
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 16), align 2, !tbaa !8
  store i16 %9, ptr getelementptr inbounds nuw (i8, ptr @d, i16 16), align 2, !tbaa !8
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 18), align 2, !tbaa !8
  store i16 %10, ptr getelementptr inbounds nuw (i8, ptr @d, i16 18), align 2, !tbaa !8
  %11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 20), align 2, !tbaa !8
  store i16 %11, ptr getelementptr inbounds nuw (i8, ptr @d, i16 20), align 2, !tbaa !8
  %12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 22), align 2, !tbaa !8
  store i16 %12, ptr getelementptr inbounds nuw (i8, ptr @d, i16 22), align 2, !tbaa !8
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 24), align 2, !tbaa !8
  store i16 %13, ptr getelementptr inbounds nuw (i8, ptr @d, i16 24), align 2, !tbaa !8
  %14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 26), align 2, !tbaa !8
  store i16 %14, ptr getelementptr inbounds nuw (i8, ptr @d, i16 26), align 2, !tbaa !8
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 28), align 2, !tbaa !8
  store i16 %15, ptr getelementptr inbounds nuw (i8, ptr @d, i16 28), align 2, !tbaa !8
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 30), align 2, !tbaa !8
  store i16 %16, ptr getelementptr inbounds nuw (i8, ptr @d, i16 30), align 2, !tbaa !8
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 32), align 2, !tbaa !8
  store i16 %17, ptr getelementptr inbounds nuw (i8, ptr @d, i16 32), align 2, !tbaa !8
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 34), align 2, !tbaa !8
  store i16 %18, ptr getelementptr inbounds nuw (i8, ptr @d, i16 34), align 2, !tbaa !8
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 36), align 2, !tbaa !8
  store i16 %19, ptr getelementptr inbounds nuw (i8, ptr @d, i16 36), align 2, !tbaa !8
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 38), align 2, !tbaa !8
  store i16 %20, ptr getelementptr inbounds nuw (i8, ptr @d, i16 38), align 2, !tbaa !8
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 40), align 2, !tbaa !8
  store i16 %21, ptr getelementptr inbounds nuw (i8, ptr @d, i16 40), align 2, !tbaa !8
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 42), align 2, !tbaa !8
  store i16 %22, ptr getelementptr inbounds nuw (i8, ptr @d, i16 42), align 2, !tbaa !8
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 44), align 2, !tbaa !8
  store i16 %23, ptr getelementptr inbounds nuw (i8, ptr @d, i16 44), align 2, !tbaa !8
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 46), align 2, !tbaa !8
  store i16 %24, ptr getelementptr inbounds nuw (i8, ptr @d, i16 46), align 2, !tbaa !8
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 48), align 2, !tbaa !8
  store i16 %25, ptr getelementptr inbounds nuw (i8, ptr @d, i16 48), align 2, !tbaa !8
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 50), align 2, !tbaa !8
  store i16 %26, ptr getelementptr inbounds nuw (i8, ptr @d, i16 50), align 2, !tbaa !8
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 52), align 2, !tbaa !8
  store i16 %27, ptr getelementptr inbounds nuw (i8, ptr @d, i16 52), align 2, !tbaa !8
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 54), align 2, !tbaa !8
  store i16 %28, ptr getelementptr inbounds nuw (i8, ptr @d, i16 54), align 2, !tbaa !8
  %29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 56), align 2, !tbaa !8
  store i16 %29, ptr getelementptr inbounds nuw (i8, ptr @d, i16 56), align 2, !tbaa !8
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 58), align 2, !tbaa !8
  store i16 %30, ptr getelementptr inbounds nuw (i8, ptr @d, i16 58), align 2, !tbaa !8
  %31 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 60), align 2, !tbaa !8
  store i16 %31, ptr getelementptr inbounds nuw (i8, ptr @d, i16 60), align 2, !tbaa !8
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 62), align 2, !tbaa !8
  store i16 %32, ptr getelementptr inbounds nuw (i8, ptr @d, i16 62), align 2, !tbaa !8
  %33 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 64), align 2, !tbaa !8
  store i16 %33, ptr getelementptr inbounds nuw (i8, ptr @d, i16 64), align 2, !tbaa !8
  %34 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 66), align 2, !tbaa !8
  store i16 %34, ptr getelementptr inbounds nuw (i8, ptr @d, i16 66), align 2, !tbaa !8
  %35 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 68), align 2, !tbaa !8
  store i16 %35, ptr getelementptr inbounds nuw (i8, ptr @d, i16 68), align 2, !tbaa !8
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 70), align 2, !tbaa !8
  store i16 %36, ptr getelementptr inbounds nuw (i8, ptr @d, i16 70), align 2, !tbaa !8
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 72), align 2, !tbaa !8
  store i16 %37, ptr getelementptr inbounds nuw (i8, ptr @d, i16 72), align 2, !tbaa !8
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 74), align 2, !tbaa !8
  store i16 %38, ptr getelementptr inbounds nuw (i8, ptr @d, i16 74), align 2, !tbaa !8
  %39 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 76), align 2, !tbaa !8
  store i16 %39, ptr getelementptr inbounds nuw (i8, ptr @d, i16 76), align 2, !tbaa !8
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 78), align 2, !tbaa !8
  store i16 %40, ptr getelementptr inbounds nuw (i8, ptr @d, i16 78), align 2, !tbaa !8
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 80), align 2, !tbaa !8
  store i16 %41, ptr getelementptr inbounds nuw (i8, ptr @d, i16 80), align 2, !tbaa !8
  %42 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 82), align 2, !tbaa !8
  store i16 %42, ptr getelementptr inbounds nuw (i8, ptr @d, i16 82), align 2, !tbaa !8
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 84), align 2, !tbaa !8
  store i16 %43, ptr getelementptr inbounds nuw (i8, ptr @d, i16 84), align 2, !tbaa !8
  %44 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 86), align 2, !tbaa !8
  store i16 %44, ptr getelementptr inbounds nuw (i8, ptr @d, i16 86), align 2, !tbaa !8
  %45 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 88), align 2, !tbaa !8
  store i16 %45, ptr getelementptr inbounds nuw (i8, ptr @d, i16 88), align 2, !tbaa !8
  %46 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 90), align 2, !tbaa !8
  store i16 %46, ptr getelementptr inbounds nuw (i8, ptr @d, i16 90), align 2, !tbaa !8
  %47 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 92), align 2, !tbaa !8
  store i16 %47, ptr getelementptr inbounds nuw (i8, ptr @d, i16 92), align 2, !tbaa !8
  %48 = load i16, ptr getelementptr inbounds nuw (i8, ptr @s, i16 94), align 2, !tbaa !8
  store i16 %48, ptr getelementptr inbounds nuw (i8, ptr @d, i16 94), align 2, !tbaa !8
  ret void
}

; Function Attrs: nounwind optsize
define internal void @smear() #0 {
  %1 = load i16, ptr @t, align 2, !tbaa !8
  store i16 %1, ptr getelementptr inbounds nuw (i8, ptr @t, i16 2), align 2, !tbaa !8
  %2 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 2), align 2, !tbaa !8
  store i16 %2, ptr getelementptr inbounds nuw (i8, ptr @t, i16 4), align 2, !tbaa !8
  %3 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 4), align 2, !tbaa !8
  store i16 %3, ptr getelementptr inbounds nuw (i8, ptr @t, i16 6), align 2, !tbaa !8
  %4 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 6), align 2, !tbaa !8
  store i16 %4, ptr getelementptr inbounds nuw (i8, ptr @t, i16 8), align 2, !tbaa !8
  %5 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 8), align 2, !tbaa !8
  store i16 %5, ptr getelementptr inbounds nuw (i8, ptr @t, i16 10), align 2, !tbaa !8
  %6 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 10), align 2, !tbaa !8
  store i16 %6, ptr getelementptr inbounds nuw (i8, ptr @t, i16 12), align 2, !tbaa !8
  %7 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 12), align 2, !tbaa !8
  store i16 %7, ptr getelementptr inbounds nuw (i8, ptr @t, i16 14), align 2, !tbaa !8
  %8 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 14), align 2, !tbaa !8
  store i16 %8, ptr getelementptr inbounds nuw (i8, ptr @t, i16 16), align 2, !tbaa !8
  %9 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 16), align 2, !tbaa !8
  store i16 %9, ptr getelementptr inbounds nuw (i8, ptr @t, i16 18), align 2, !tbaa !8
  %10 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 18), align 2, !tbaa !8
  store i16 %10, ptr getelementptr inbounds nuw (i8, ptr @t, i16 20), align 2, !tbaa !8
  %11 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 20), align 2, !tbaa !8
  store i16 %11, ptr getelementptr inbounds nuw (i8, ptr @t, i16 22), align 2, !tbaa !8
  %12 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 22), align 2, !tbaa !8
  store i16 %12, ptr getelementptr inbounds nuw (i8, ptr @t, i16 24), align 2, !tbaa !8
  %13 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 24), align 2, !tbaa !8
  store i16 %13, ptr getelementptr inbounds nuw (i8, ptr @t, i16 26), align 2, !tbaa !8
  %14 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 26), align 2, !tbaa !8
  store i16 %14, ptr getelementptr inbounds nuw (i8, ptr @t, i16 28), align 2, !tbaa !8
  %15 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 28), align 2, !tbaa !8
  store i16 %15, ptr getelementptr inbounds nuw (i8, ptr @t, i16 30), align 2, !tbaa !8
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 30), align 2, !tbaa !8
  store i16 %16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 32), align 2, !tbaa !8
  %17 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 32), align 2, !tbaa !8
  store i16 %17, ptr getelementptr inbounds nuw (i8, ptr @t, i16 34), align 2, !tbaa !8
  %18 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 34), align 2, !tbaa !8
  store i16 %18, ptr getelementptr inbounds nuw (i8, ptr @t, i16 36), align 2, !tbaa !8
  %19 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 36), align 2, !tbaa !8
  store i16 %19, ptr getelementptr inbounds nuw (i8, ptr @t, i16 38), align 2, !tbaa !8
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 38), align 2, !tbaa !8
  store i16 %20, ptr getelementptr inbounds nuw (i8, ptr @t, i16 40), align 2, !tbaa !8
  %21 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 40), align 2, !tbaa !8
  store i16 %21, ptr getelementptr inbounds nuw (i8, ptr @t, i16 42), align 2, !tbaa !8
  %22 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 42), align 2, !tbaa !8
  store i16 %22, ptr getelementptr inbounds nuw (i8, ptr @t, i16 44), align 2, !tbaa !8
  %23 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 44), align 2, !tbaa !8
  store i16 %23, ptr getelementptr inbounds nuw (i8, ptr @t, i16 46), align 2, !tbaa !8
  %24 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 46), align 2, !tbaa !8
  store i16 %24, ptr getelementptr inbounds nuw (i8, ptr @t, i16 48), align 2, !tbaa !8
  %25 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 48), align 2, !tbaa !8
  store i16 %25, ptr getelementptr inbounds nuw (i8, ptr @t, i16 50), align 2, !tbaa !8
  %26 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 50), align 2, !tbaa !8
  store i16 %26, ptr getelementptr inbounds nuw (i8, ptr @t, i16 52), align 2, !tbaa !8
  %27 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 52), align 2, !tbaa !8
  store i16 %27, ptr getelementptr inbounds nuw (i8, ptr @t, i16 54), align 2, !tbaa !8
  %28 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 54), align 2, !tbaa !8
  store i16 %28, ptr getelementptr inbounds nuw (i8, ptr @t, i16 56), align 2, !tbaa !8
  %29 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 56), align 2, !tbaa !8
  store i16 %29, ptr getelementptr inbounds nuw (i8, ptr @t, i16 58), align 2, !tbaa !8
  %30 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 58), align 2, !tbaa !8
  store i16 %30, ptr getelementptr inbounds nuw (i8, ptr @t, i16 60), align 2, !tbaa !8
  %31 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 60), align 2, !tbaa !8
  store i16 %31, ptr getelementptr inbounds nuw (i8, ptr @t, i16 62), align 2, !tbaa !8
  %32 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 62), align 2, !tbaa !8
  store i16 %32, ptr getelementptr inbounds nuw (i8, ptr @t, i16 64), align 2, !tbaa !8
  %33 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 64), align 2, !tbaa !8
  store i16 %33, ptr getelementptr inbounds nuw (i8, ptr @t, i16 66), align 2, !tbaa !8
  %34 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 66), align 2, !tbaa !8
  store i16 %34, ptr getelementptr inbounds nuw (i8, ptr @t, i16 68), align 2, !tbaa !8
  %35 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 68), align 2, !tbaa !8
  store i16 %35, ptr getelementptr inbounds nuw (i8, ptr @t, i16 70), align 2, !tbaa !8
  %36 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 70), align 2, !tbaa !8
  store i16 %36, ptr getelementptr inbounds nuw (i8, ptr @t, i16 72), align 2, !tbaa !8
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 72), align 2, !tbaa !8
  store i16 %37, ptr getelementptr inbounds nuw (i8, ptr @t, i16 74), align 2, !tbaa !8
  %38 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 74), align 2, !tbaa !8
  store i16 %38, ptr getelementptr inbounds nuw (i8, ptr @t, i16 76), align 2, !tbaa !8
  %39 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 76), align 2, !tbaa !8
  store i16 %39, ptr getelementptr inbounds nuw (i8, ptr @t, i16 78), align 2, !tbaa !8
  %40 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 78), align 2, !tbaa !8
  store i16 %40, ptr getelementptr inbounds nuw (i8, ptr @t, i16 80), align 2, !tbaa !8
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 80), align 2, !tbaa !8
  store i16 %41, ptr getelementptr inbounds nuw (i8, ptr @t, i16 82), align 2, !tbaa !8
  %42 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 82), align 2, !tbaa !8
  store i16 %42, ptr getelementptr inbounds nuw (i8, ptr @t, i16 84), align 2, !tbaa !8
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 84), align 2, !tbaa !8
  store i16 %43, ptr getelementptr inbounds nuw (i8, ptr @t, i16 86), align 2, !tbaa !8
  %44 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 86), align 2, !tbaa !8
  store i16 %44, ptr getelementptr inbounds nuw (i8, ptr @t, i16 88), align 2, !tbaa !8
  %45 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 88), align 2, !tbaa !8
  store i16 %45, ptr getelementptr inbounds nuw (i8, ptr @t, i16 90), align 2, !tbaa !8
  %46 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 90), align 2, !tbaa !8
  store i16 %46, ptr getelementptr inbounds nuw (i8, ptr @t, i16 92), align 2, !tbaa !8
  %47 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 92), align 2, !tbaa !8
  store i16 %47, ptr getelementptr inbounds nuw (i8, ptr @t, i16 94), align 2, !tbaa !8
  %48 = load i16, ptr getelementptr inbounds nuw (i8, ptr @t, i16 94), align 2, !tbaa !8
  store i16 %48, ptr getelementptr inbounds nuw (i8, ptr @t, i16 96), align 2, !tbaa !8
  ret void
}

; Function Attrs: nounwind optsize
define internal void @narrow() #0 {
  %1 = load i8, ptr @v, align 1, !tbaa !10
  store i8 %1, ptr @w, align 1, !tbaa !10
  %2 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 1), align 1, !tbaa !10
  store i8 %2, ptr getelementptr inbounds nuw (i8, ptr @w, i16 1), align 1, !tbaa !10
  %3 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 2), align 1, !tbaa !10
  store i8 %3, ptr getelementptr inbounds nuw (i8, ptr @w, i16 2), align 1, !tbaa !10
  %4 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 3), align 1, !tbaa !10
  store i8 %4, ptr getelementptr inbounds nuw (i8, ptr @w, i16 3), align 1, !tbaa !10
  %5 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 4), align 1, !tbaa !10
  store i8 %5, ptr getelementptr inbounds nuw (i8, ptr @w, i16 4), align 1, !tbaa !10
  %6 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 5), align 1, !tbaa !10
  store i8 %6, ptr getelementptr inbounds nuw (i8, ptr @w, i16 5), align 1, !tbaa !10
  %7 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 6), align 1, !tbaa !10
  store i8 %7, ptr getelementptr inbounds nuw (i8, ptr @w, i16 6), align 1, !tbaa !10
  %8 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 7), align 1, !tbaa !10
  store i8 %8, ptr getelementptr inbounds nuw (i8, ptr @w, i16 7), align 1, !tbaa !10
  %9 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 8), align 1, !tbaa !10
  store i8 %9, ptr getelementptr inbounds nuw (i8, ptr @w, i16 8), align 1, !tbaa !10
  %10 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 9), align 1, !tbaa !10
  store i8 %10, ptr getelementptr inbounds nuw (i8, ptr @w, i16 9), align 1, !tbaa !10
  %11 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 10), align 1, !tbaa !10
  store i8 %11, ptr getelementptr inbounds nuw (i8, ptr @w, i16 10), align 1, !tbaa !10
  %12 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 11), align 1, !tbaa !10
  store i8 %12, ptr getelementptr inbounds nuw (i8, ptr @w, i16 11), align 1, !tbaa !10
  %13 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 12), align 1, !tbaa !10
  store i8 %13, ptr getelementptr inbounds nuw (i8, ptr @w, i16 12), align 1, !tbaa !10
  %14 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 13), align 1, !tbaa !10
  store i8 %14, ptr getelementptr inbounds nuw (i8, ptr @w, i16 13), align 1, !tbaa !10
  %15 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 14), align 1, !tbaa !10
  store i8 %15, ptr getelementptr inbounds nuw (i8, ptr @w, i16 14), align 1, !tbaa !10
  %16 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 15), align 1, !tbaa !10
  store i8 %16, ptr getelementptr inbounds nuw (i8, ptr @w, i16 15), align 1, !tbaa !10
  %17 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 16), align 1, !tbaa !10
  store i8 %17, ptr getelementptr inbounds nuw (i8, ptr @w, i16 16), align 1, !tbaa !10
  %18 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 17), align 1, !tbaa !10
  store i8 %18, ptr getelementptr inbounds nuw (i8, ptr @w, i16 17), align 1, !tbaa !10
  %19 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 18), align 1, !tbaa !10
  store i8 %19, ptr getelementptr inbounds nuw (i8, ptr @w, i16 18), align 1, !tbaa !10
  %20 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 19), align 1, !tbaa !10
  store i8 %20, ptr getelementptr inbounds nuw (i8, ptr @w, i16 19), align 1, !tbaa !10
  %21 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 20), align 1, !tbaa !10
  store i8 %21, ptr getelementptr inbounds nuw (i8, ptr @w, i16 20), align 1, !tbaa !10
  %22 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 21), align 1, !tbaa !10
  store i8 %22, ptr getelementptr inbounds nuw (i8, ptr @w, i16 21), align 1, !tbaa !10
  %23 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 22), align 1, !tbaa !10
  store i8 %23, ptr getelementptr inbounds nuw (i8, ptr @w, i16 22), align 1, !tbaa !10
  %24 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 23), align 1, !tbaa !10
  store i8 %24, ptr getelementptr inbounds nuw (i8, ptr @w, i16 23), align 1, !tbaa !10
  %25 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 24), align 1, !tbaa !10
  store i8 %25, ptr getelementptr inbounds nuw (i8, ptr @w, i16 24), align 1, !tbaa !10
  %26 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 25), align 1, !tbaa !10
  store i8 %26, ptr getelementptr inbounds nuw (i8, ptr @w, i16 25), align 1, !tbaa !10
  %27 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 26), align 1, !tbaa !10
  store i8 %27, ptr getelementptr inbounds nuw (i8, ptr @w, i16 26), align 1, !tbaa !10
  %28 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 27), align 1, !tbaa !10
  store i8 %28, ptr getelementptr inbounds nuw (i8, ptr @w, i16 27), align 1, !tbaa !10
  %29 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 28), align 1, !tbaa !10
  store i8 %29, ptr getelementptr inbounds nuw (i8, ptr @w, i16 28), align 1, !tbaa !10
  %30 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 29), align 1, !tbaa !10
  store i8 %30, ptr getelementptr inbounds nuw (i8, ptr @w, i16 29), align 1, !tbaa !10
  %31 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 30), align 1, !tbaa !10
  store i8 %31, ptr getelementptr inbounds nuw (i8, ptr @w, i16 30), align 1, !tbaa !10
  %32 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 31), align 1, !tbaa !10
  store i8 %32, ptr getelementptr inbounds nuw (i8, ptr @w, i16 31), align 1, !tbaa !10
  %33 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 32), align 1, !tbaa !10
  store i8 %33, ptr getelementptr inbounds nuw (i8, ptr @w, i16 32), align 1, !tbaa !10
  %34 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 33), align 1, !tbaa !10
  store i8 %34, ptr getelementptr inbounds nuw (i8, ptr @w, i16 33), align 1, !tbaa !10
  %35 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 34), align 1, !tbaa !10
  store i8 %35, ptr getelementptr inbounds nuw (i8, ptr @w, i16 34), align 1, !tbaa !10
  %36 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 35), align 1, !tbaa !10
  store i8 %36, ptr getelementptr inbounds nuw (i8, ptr @w, i16 35), align 1, !tbaa !10
  %37 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 36), align 1, !tbaa !10
  store i8 %37, ptr getelementptr inbounds nuw (i8, ptr @w, i16 36), align 1, !tbaa !10
  %38 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 37), align 1, !tbaa !10
  store i8 %38, ptr getelementptr inbounds nuw (i8, ptr @w, i16 37), align 1, !tbaa !10
  %39 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 38), align 1, !tbaa !10
  store i8 %39, ptr getelementptr inbounds nuw (i8, ptr @w, i16 38), align 1, !tbaa !10
  %40 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 39), align 1, !tbaa !10
  store i8 %40, ptr getelementptr inbounds nuw (i8, ptr @w, i16 39), align 1, !tbaa !10
  %41 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 40), align 1, !tbaa !10
  store i8 %41, ptr getelementptr inbounds nuw (i8, ptr @w, i16 40), align 1, !tbaa !10
  %42 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 41), align 1, !tbaa !10
  store i8 %42, ptr getelementptr inbounds nuw (i8, ptr @w, i16 41), align 1, !tbaa !10
  %43 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 42), align 1, !tbaa !10
  store i8 %43, ptr getelementptr inbounds nuw (i8, ptr @w, i16 42), align 1, !tbaa !10
  %44 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 43), align 1, !tbaa !10
  store i8 %44, ptr getelementptr inbounds nuw (i8, ptr @w, i16 43), align 1, !tbaa !10
  %45 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 44), align 1, !tbaa !10
  store i8 %45, ptr getelementptr inbounds nuw (i8, ptr @w, i16 44), align 1, !tbaa !10
  %46 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 45), align 1, !tbaa !10
  store i8 %46, ptr getelementptr inbounds nuw (i8, ptr @w, i16 45), align 1, !tbaa !10
  %47 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 46), align 1, !tbaa !10
  store i8 %47, ptr getelementptr inbounds nuw (i8, ptr @w, i16 46), align 1, !tbaa !10
  %48 = load i8, ptr getelementptr inbounds nuw (i8, ptr @v, i16 47), align 1, !tbaa !10
  store i8 %48, ptr getelementptr inbounds nuw (i8, ptr @w, i16 47), align 1, !tbaa !10
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @sum(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !13
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %21, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %24

11:                                               ; preds = %7
  %12 = load i32, ptr %5, align 2, !tbaa !16
  %13 = mul nsw i32 %12, 3
  %14 = load ptr, ptr %3, align 2, !tbaa !13
  %15 = load i16, ptr %6, align 2, !tbaa !7
  %16 = getelementptr inbounds i16, ptr %14, i16 %15
  %17 = load i16, ptr %16, align 2, !tbaa !8
  %18 = sext i16 %17 to i32
  %19 = add nsw i32 %13, %18
  %20 = srem i32 %19, 1000003
  store i32 %20, ptr %5, align 2, !tbaa !16
  br label %21

21:                                               ; preds = %11
  %22 = load i16, ptr %6, align 2, !tbaa !7
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !18

24:                                               ; preds = %7
  %25 = load i32, ptr %5, align 2, !tbaa !16
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %25
}

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
!9 = !{!"short", !5, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!14, !14, i64 0}
!14 = !{!"p1 short", !15, i64 0}
!15 = !{!"any pointer", !5, i64 0}
!16 = !{!17, !17, i64 0}
!17 = !{!"long", !5, i64 0}
!18 = distinct !{!18, !12}
