target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'word_sign_zero_guard.c'
source_filename = "word_sign_zero_guard.c"
target triple = "msp430"

@taken = internal global i16 0, align 2
@main.v = internal constant [12 x i16] [i16 0, i16 1, i16 -1, i16 2, i16 -2, i16 255, i16 256, i16 -256, i16 32767, i16 -32768, i16 16384, i16 -16384], align 2

; Function Attrs: noinline nounwind
define dso_local void @yes() #0 {
  store i16 7, ptr @taken, align 2, !tbaa !7
  ret void
}

; Function Attrs: noinline nounwind
define dso_local void @no() #0 {
  store i16 3, ptr @taken, align 2, !tbaa !7
  ret void
}

; Function Attrs: noinline nounwind
define dso_local i16 @gt0(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sgt i16 %3, 0
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  call void @yes()
  br label %7

6:                                                ; preds = %1
  call void @no()
  br label %7

7:                                                ; preds = %6, %5
  %8 = load i16, ptr @taken, align 2, !tbaa !7
  ret i16 %8
}

; Function Attrs: noinline nounwind
define dso_local i16 @le0(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sle i16 %3, 0
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  call void @yes()
  br label %7

6:                                                ; preds = %1
  call void @no()
  br label %7

7:                                                ; preds = %6, %5
  %8 = load i16, ptr @taken, align 2, !tbaa !7
  ret i16 %8
}

; Function Attrs: noinline nounwind
define dso_local i16 @ge1(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sge i16 %3, 1
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  call void @yes()
  br label %7

6:                                                ; preds = %1
  call void @no()
  br label %7

7:                                                ; preds = %6, %5
  %8 = load i16, ptr @taken, align 2, !tbaa !7
  ret i16 %8
}

; Function Attrs: noinline nounwind
define dso_local i16 @lt1(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp slt i16 %3, 1
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  call void @yes()
  br label %7

6:                                                ; preds = %1
  call void @no()
  br label %7

7:                                                ; preds = %6, %5
  %8 = load i16, ptr @taken, align 2, !tbaa !7
  ret i16 %8
}

; Function Attrs: noinline nounwind
define dso_local i16 @sel_gt0(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sgt i16 %3, 0
  %5 = zext i1 %4 to i64
  %6 = select i1 %4, i16 7, i16 3
  ret i16 %6
}

; Function Attrs: noinline nounwind
define dso_local i16 @sel_le0(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sle i16 %3, 0
  %5 = zext i1 %4 to i64
  %6 = select i1 %4, i16 7, i16 3
  ret i16 %6
}

; Function Attrs: noinline nounwind
define dso_local i16 @bool_ge1(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp sge i16 %3, 1
  %5 = zext i1 %4 to i16
  ret i16 %5
}

; Function Attrs: noinline nounwind
define dso_local i16 @bool_lt1(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  %3 = load i16, ptr %2, align 2, !tbaa !8
  %4 = icmp slt i16 %3, 1
  %5 = zext i1 %4 to i16
  ret i16 %5
}

; Function Attrs: nounwind
define dso_local i16 @main() #1 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %54, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp ult i16 %4, 12
  br i1 %5, label %7, label %6

6:                                                ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #4
  br label %57

7:                                                ; preds = %3
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %8
  %10 = load i16, ptr %9, align 2, !tbaa !8
  %11 = call i16 @gt0(i16 noundef signext %10)
  %12 = mul nsw i16 %11, 1000
  %13 = load i16, ptr %2, align 2, !tbaa !7
  %14 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %13
  %15 = load i16, ptr %14, align 2, !tbaa !8
  %16 = call i16 @le0(i16 noundef signext %15)
  %17 = mul nsw i16 %16, 100
  %18 = add nsw i16 %12, %17
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %19
  %21 = load i16, ptr %20, align 2, !tbaa !8
  %22 = call i16 @ge1(i16 noundef signext %21)
  %23 = mul nsw i16 %22, 10
  %24 = add nsw i16 %18, %23
  %25 = load i16, ptr %2, align 2, !tbaa !7
  %26 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %25
  %27 = load i16, ptr %26, align 2, !tbaa !8
  %28 = call i16 @lt1(i16 noundef signext %27)
  %29 = add nsw i16 %24, %28
  %30 = sext i16 %29 to i32
  call void @report(i32 noundef %30)
  %31 = load i16, ptr %2, align 2, !tbaa !7
  %32 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %31
  %33 = load i16, ptr %32, align 2, !tbaa !8
  %34 = call i16 @sel_gt0(i16 noundef signext %33)
  %35 = mul nsw i16 %34, 1000
  %36 = load i16, ptr %2, align 2, !tbaa !7
  %37 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %36
  %38 = load i16, ptr %37, align 2, !tbaa !8
  %39 = call i16 @sel_le0(i16 noundef signext %38)
  %40 = mul nsw i16 %39, 100
  %41 = add nsw i16 %35, %40
  %42 = load i16, ptr %2, align 2, !tbaa !7
  %43 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %42
  %44 = load i16, ptr %43, align 2, !tbaa !8
  %45 = call i16 @bool_ge1(i16 noundef signext %44)
  %46 = mul nsw i16 %45, 10
  %47 = add nsw i16 %41, %46
  %48 = load i16, ptr %2, align 2, !tbaa !7
  %49 = getelementptr inbounds nuw [12 x i16], ptr @main.v, i16 0, i16 %48
  %50 = load i16, ptr %49, align 2, !tbaa !8
  %51 = call i16 @bool_lt1(i16 noundef signext %50)
  %52 = add nsw i16 %47, %51
  %53 = sext i16 %52 to i32
  call void @report(i32 noundef %53)
  br label %54

54:                                               ; preds = %7
  %55 = load i16, ptr %2, align 2, !tbaa !7
  %56 = add i16 %55, 1
  store i16 %56, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !10

57:                                               ; preds = %6
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

declare dso_local void @report(i32 noundef) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"short", !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
