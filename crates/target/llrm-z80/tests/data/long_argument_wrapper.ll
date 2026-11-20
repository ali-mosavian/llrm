target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@main.v = internal constant [4 x i64] [i64 1311768467463790320, i64 -9223372036854775807, i64 -1, i64 71777214530522895], align 2
@main.counts = internal constant [14 x i16] [i16 0, i16 1, i16 2, i16 7, i16 8, i16 9, i16 15, i16 16, i16 31, i16 32, i16 33, i16 47, i16 48, i16 63], align 2

; Function Attrs: noinline nounwind optsize
define dso_local i64 @shl(i64 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i16, align 2
  store i64 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i64, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = zext i16 %6 to i64
  %8 = shl i64 %5, %7
  ret i64 %8
}

; Function Attrs: noinline nounwind optsize
define dso_local i64 @shr(i64 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i16, align 2
  store i64 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i64, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = zext i16 %6 to i64
  %8 = lshr i64 %5, %7
  ret i64 %8
}

; Function Attrs: noinline nounwind optsize
define dso_local i64 @sar(i64 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i64, align 2
  %4 = alloca i16, align 2
  store i64 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i64, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = zext i16 %6 to i64
  %8 = ashr i64 %5, %7
  ret i64 %8
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #1 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %6

6:                                                ; preds = %38, %0
  %7 = load i16, ptr %3, align 2, !tbaa !9
  %8 = icmp ult i16 %7, 4
  br i1 %8, label %10, label %9

9:                                                ; preds = %6
  store i32 2, ptr %4, align 2
  call void @llvm.lifetime.end.p0(ptr %3) #4
  br label %41

10:                                               ; preds = %6
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %11

11:                                               ; preds = %34, %10
  %12 = load i16, ptr %5, align 2, !tbaa !9
  %13 = icmp ult i16 %12, 14
  br i1 %13, label %15, label %14

14:                                               ; preds = %11
  store i32 5, ptr %4, align 2
  call void @llvm.lifetime.end.p0(ptr %5) #4
  br label %37

15:                                               ; preds = %11
  %16 = load i16, ptr %5, align 2, !tbaa !9
  %17 = getelementptr inbounds nuw [14 x i16], ptr @main.counts, i16 0, i16 %16
  %18 = load i16, ptr %17, align 2, !tbaa !9
  store volatile i16 %18, ptr %2, align 2, !tbaa !9
  %19 = load i16, ptr %3, align 2, !tbaa !9
  %20 = getelementptr inbounds nuw [4 x i64], ptr @main.v, i16 0, i16 %19
  %21 = load i64, ptr %20, align 2, !tbaa !7
  %22 = load volatile i16, ptr %2, align 2, !tbaa !9
  %23 = call i64 @shl(i64 noundef %21, i16 noundef %22) #5
  call void @put(i64 noundef %23) #5
  %24 = load i16, ptr %3, align 2, !tbaa !9
  %25 = getelementptr inbounds nuw [4 x i64], ptr @main.v, i16 0, i16 %24
  %26 = load i64, ptr %25, align 2, !tbaa !7
  %27 = load volatile i16, ptr %2, align 2, !tbaa !9
  %28 = call i64 @shr(i64 noundef %26, i16 noundef %27) #5
  call void @put(i64 noundef %28) #5
  %29 = load i16, ptr %3, align 2, !tbaa !9
  %30 = getelementptr inbounds nuw [4 x i64], ptr @main.v, i16 0, i16 %29
  %31 = load i64, ptr %30, align 2, !tbaa !7
  %32 = load volatile i16, ptr %2, align 2, !tbaa !9
  %33 = call i64 @sar(i64 noundef %31, i16 noundef %32) #5
  call void @put(i64 noundef %33) #5
  br label %34

34:                                               ; preds = %15
  %35 = load i16, ptr %5, align 2, !tbaa !9
  %36 = add i16 %35, 1
  store i16 %36, ptr %5, align 2, !tbaa !9
  br label %11, !llvm.loop !10

37:                                               ; preds = %14
  br label %38

38:                                               ; preds = %37
  %39 = load i16, ptr %3, align 2, !tbaa !9
  %40 = add i16 %39, 1
  store i16 %40, ptr %3, align 2, !tbaa !9
  br label %6, !llvm.loop !12

41:                                               ; preds = %9
  store i16 0, ptr %1, align 2
  store i32 1, ptr %4, align 2
  call void @llvm.lifetime.end.p0(ptr %2) #4
  %42 = load i16, ptr %1, align 2
  ret i16 %42
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nounwind optsize
define internal void @put(i64 noundef %0) #1 {
  %2 = alloca i64, align 2
  store i64 %0, ptr %2, align 2, !tbaa !7
  %3 = load i64, ptr %2, align 2, !tbaa !7
  %4 = lshr i64 %3, 32
  %5 = trunc i64 %4 to i32
  call void @report(i32 noundef %5) #5
  %6 = load i64, ptr %2, align 2, !tbaa !7
  %7 = trunc i64 %6 to i32
  call void @report(i32 noundef %7) #5
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

attributes #0 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

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
!8 = !{!"long long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
