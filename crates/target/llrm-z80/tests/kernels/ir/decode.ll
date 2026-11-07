source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@maxcode = dso_local global [4 x [18 x i16]] zeroinitializer, align 2
@bad = external dso_local global i16, align 2
@huffval = dso_local global [4 x [256 x i8]] zeroinitializer, align 1
@valptr = dso_local global [4 x [17 x i16]] zeroinitializer, align 2
@mincode = dso_local global [4 x [17 x i16]] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i16 @decode(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %7 = call i16 @getbit()
  store i16 %7, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 1, ptr %5, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %25, %1
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = getelementptr inbounds [4 x [18 x i16]], ptr @maxcode, i16 0, i16 %10
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [18 x i16], ptr %11, i16 0, i16 %12
  %14 = load i16, ptr %13, align 2, !tbaa !7
  %15 = icmp sgt i16 %9, %14
  br i1 %15, label %16, label %26

16:                                               ; preds = %8
  %17 = load i16, ptr %4, align 2, !tbaa !7
  %18 = mul nsw i16 %17, 2
  %19 = call i16 @getbit()
  %20 = add nsw i16 %18, %19
  store i16 %20, ptr %4, align 2, !tbaa !7
  %21 = load i16, ptr %5, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %5, align 2, !tbaa !7
  %23 = icmp sgt i16 %22, 16
  br i1 %23, label %24, label %25

24:                                               ; preds = %16
  store i16 1, ptr @bad, align 2, !tbaa !7
  store i16 0, ptr %2, align 2
  store i32 1, ptr %6, align 2
  br label %45

25:                                               ; preds = %16
  br label %8, !llvm.loop !8

26:                                               ; preds = %8
  %27 = load i16, ptr %3, align 2, !tbaa !7
  %28 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i16 0, i16 %27
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = getelementptr inbounds [4 x [17 x i16]], ptr @valptr, i16 0, i16 %29
  %31 = load i16, ptr %5, align 2, !tbaa !7
  %32 = getelementptr inbounds [17 x i16], ptr %30, i16 0, i16 %31
  %33 = load i16, ptr %32, align 2, !tbaa !7
  %34 = load i16, ptr %4, align 2, !tbaa !7
  %35 = add nsw i16 %33, %34
  %36 = load i16, ptr %3, align 2, !tbaa !7
  %37 = getelementptr inbounds [4 x [17 x i16]], ptr @mincode, i16 0, i16 %36
  %38 = load i16, ptr %5, align 2, !tbaa !7
  %39 = getelementptr inbounds [17 x i16], ptr %37, i16 0, i16 %38
  %40 = load i16, ptr %39, align 2, !tbaa !7
  %41 = sub nsw i16 %35, %40
  %42 = getelementptr inbounds [256 x i8], ptr %28, i16 0, i16 %41
  %43 = load i8, ptr %42, align 1, !tbaa !10
  %44 = zext i8 %43 to i16
  store i16 %44, ptr %2, align 2
  store i32 1, ptr %6, align 2
  br label %45

45:                                               ; preds = %26, %24
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  %46 = load i16, ptr %2, align 2
  ret i16 %46
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local i16 @getbit() #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!5, !5, i64 0}
