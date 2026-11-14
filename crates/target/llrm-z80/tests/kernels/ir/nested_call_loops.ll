target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@v = internal constant [4 x i32] [i32 0, i32 1, i32 -1, i32 32767], align 2

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca [2 x i32], align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %53, %0
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = icmp ult i16 %8, 2
  br i1 %9, label %10, label %56

10:                                               ; preds = %7
  %11 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 1
  store i32 0, ptr %11, align 2, !tbaa !8
  %12 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 0
  store i32 0, ptr %12, align 2, !tbaa !8
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %13

13:                                               ; preds = %45, %10
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = icmp ult i16 %14, 4
  br i1 %15, label %16, label %48

16:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(ptr %5) #4
  %17 = load i16, ptr %2, align 2, !tbaa !7
  %18 = getelementptr inbounds nuw [4 x i32], ptr @v, i16 0, i16 %17
  %19 = load i32, ptr %18, align 2, !tbaa !8
  store i32 %19, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = getelementptr inbounds nuw [4 x i32], ptr @v, i16 0, i16 %20
  %22 = load i32, ptr %21, align 2, !tbaa !8
  store i32 %22, ptr %6, align 2, !tbaa !8
  %23 = load i32, ptr %5, align 2, !tbaa !8
  %24 = load i32, ptr %6, align 2, !tbaa !8
  %25 = call i16 @slt(i32 noundef %23, i32 noundef %24)
  %26 = sext i16 %25 to i32
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = mul i16 2, %27
  %29 = zext i16 %28 to i32
  %30 = shl i32 %26, %29
  %31 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 0
  %32 = load i32, ptr %31, align 2, !tbaa !8
  %33 = or i32 %32, %30
  store i32 %33, ptr %31, align 2, !tbaa !8
  %34 = load i32, ptr %5, align 2, !tbaa !8
  %35 = load i32, ptr %6, align 2, !tbaa !8
  %36 = call i16 @sge(i32 noundef %34, i32 noundef %35)
  %37 = sext i16 %36 to i32
  %38 = load i16, ptr %3, align 2, !tbaa !7
  %39 = mul i16 2, %38
  %40 = zext i16 %39 to i32
  %41 = shl i32 %37, %40
  %42 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 1
  %43 = load i32, ptr %42, align 2, !tbaa !8
  %44 = or i32 %43, %41
  store i32 %44, ptr %42, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  br label %45

45:                                               ; preds = %16
  %46 = load i16, ptr %3, align 2, !tbaa !7
  %47 = add i16 %46, 1
  store i16 %47, ptr %3, align 2, !tbaa !7
  br label %13, !llvm.loop !10

48:                                               ; preds = %13
  %49 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 0
  %50 = load i32, ptr %49, align 2, !tbaa !8
  call void @report(i32 noundef %50)
  %51 = getelementptr inbounds [2 x i32], ptr %4, i16 0, i16 1
  %52 = load i32, ptr %51, align 2, !tbaa !8
  call void @report(i32 noundef %52)
  br label %53

53:                                               ; preds = %48
  %54 = load i16, ptr %2, align 2, !tbaa !7
  %55 = add i16 %54, 1
  store i16 %55, ptr %2, align 2, !tbaa !7
  br label %7, !llvm.loop !12

56:                                               ; preds = %7
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind
define internal i16 @slt(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = icmp slt i32 %5, %6
  %8 = zext i1 %7 to i16
  ret i16 %8
}

; Function Attrs: noinline nounwind
define internal i16 @sge(i32 noundef %0, i32 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = icmp sge i32 %5, %6
  %8 = zext i1 %7 to i16
  ret i16 %8
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #3

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
