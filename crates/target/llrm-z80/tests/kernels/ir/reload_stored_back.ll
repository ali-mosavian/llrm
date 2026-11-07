target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 'm2.c'
source_filename = "m2.c"
target triple = "msp430"

@dv = external dso_local global i32, align 2

; Function Attrs: noinline nounwind optsize
define dso_local i32 @g(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #2
  store i32 0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %4) #2
  store i32 0, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %61, %1
  %10 = load i16, ptr %5, align 2, !tbaa !7
  %11 = load i16, ptr %2, align 2, !tbaa !7
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %14, label %13

13:                                               ; preds = %9
  call void @llvm.lifetime.end.p0(ptr %5) #2
  br label %64

14:                                               ; preds = %9
  call void @llvm.lifetime.start.p0(ptr %6) #2
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = icmp slt i16 %15, 2
  br i1 %16, label %17, label %20

17:                                               ; preds = %14
  %18 = load i16, ptr %5, align 2, !tbaa !7
  %19 = sext i16 %18 to i32
  br label %34

20:                                               ; preds = %14
  %21 = load i16, ptr %5, align 2, !tbaa !7
  %22 = icmp slt i16 %21, 4
  br i1 %22, label %23, label %28

23:                                               ; preds = %20
  %24 = load i16, ptr %5, align 2, !tbaa !7
  %25 = sub nsw i16 %24, 2
  %26 = sext i16 %25 to i32
  %27 = sub nsw i32 2147483647, %26
  br label %32

28:                                               ; preds = %20
  %29 = load i16, ptr %5, align 2, !tbaa !7
  %30 = sub nsw i16 %29, 5
  %31 = sext i16 %30 to i32
  br label %32

32:                                               ; preds = %28, %23
  %33 = phi i32 [ %27, %23 ], [ %31, %28 ]
  br label %34

34:                                               ; preds = %32, %17
  %35 = phi i32 [ %19, %17 ], [ %33, %32 ]
  store i32 %35, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %36 = load i32, ptr %6, align 2, !tbaa !8
  %37 = sdiv i32 %36, 7
  store i32 %37, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %38 = load i32, ptr %6, align 2, !tbaa !8
  %39 = srem i32 %38, 7
  store i32 %39, ptr %8, align 2, !tbaa !8
  %40 = load i32, ptr %7, align 2, !tbaa !8
  %41 = load i32, ptr %6, align 2, !tbaa !8
  %42 = load volatile i32, ptr @dv, align 2, !tbaa !8
  %43 = sdiv i32 %41, %42
  %44 = icmp ne i32 %40, %43
  br i1 %44, label %51, label %45

45:                                               ; preds = %34
  %46 = load i32, ptr %8, align 2, !tbaa !8
  %47 = load i32, ptr %6, align 2, !tbaa !8
  %48 = load volatile i32, ptr @dv, align 2, !tbaa !8
  %49 = srem i32 %47, %48
  %50 = icmp ne i32 %46, %49
  br i1 %50, label %51, label %54

51:                                               ; preds = %45, %34
  %52 = load i32, ptr %3, align 2, !tbaa !8
  %53 = add nsw i32 %52, 1
  store i32 %53, ptr %3, align 2, !tbaa !8
  br label %54

54:                                               ; preds = %51, %45
  %55 = load i32, ptr %7, align 2, !tbaa !8
  %56 = mul nsw i32 %55, 31
  %57 = load i32, ptr %8, align 2, !tbaa !8
  %58 = add nsw i32 %56, %57
  %59 = load i32, ptr %4, align 2, !tbaa !8
  %60 = add nsw i32 %59, %58
  store i32 %60, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  call void @llvm.lifetime.end.p0(ptr %6) #2
  br label %61

61:                                               ; preds = %54
  %62 = load i16, ptr %5, align 2, !tbaa !7
  %63 = add nsw i16 %62, 1
  store i16 %63, ptr %5, align 2, !tbaa !7
  br label %9, !llvm.loop !10

64:                                               ; preds = %13
  %65 = load i32, ptr %3, align 2, !tbaa !8
  %66 = load i32, ptr %4, align 2, !tbaa !8
  %67 = add nsw i32 %65, %66
  call void @llvm.lifetime.end.p0(ptr %4) #2
  call void @llvm.lifetime.end.p0(ptr %3) #2
  ret i32 %67
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
