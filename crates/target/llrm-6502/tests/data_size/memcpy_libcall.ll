target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

%struct.big = type { i16, [196 x i8], i16 }

@from = internal global [300 x i8] zeroinitializer, align 1
@src = internal global %struct.big zeroinitializer, align 1
@dst = internal global %struct.big zeroinitializer, align 1
@to = internal global [300 x i8] zeroinitializer, align 1
@off = internal global i16 1, align 2
@n_var = internal global i16 150, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %4

4:                                                ; preds = %14, %0
  %5 = load i16, ptr %3, align 2, !tbaa !9
  %6 = icmp ult i16 %5, 300
  br i1 %6, label %7, label %17

7:                                                ; preds = %4
  %8 = load i16, ptr %3, align 2, !tbaa !9
  %9 = mul i16 %8, 5
  %10 = add i16 %9, 1
  %11 = trunc i16 %10 to i8
  %12 = load i16, ptr %3, align 2, !tbaa !9
  %13 = getelementptr inbounds nuw [300 x i8], ptr @from, i16 0, i16 %12
  store i8 %11, ptr %13, align 1, !tbaa !10
  br label %14

14:                                               ; preds = %7
  %15 = load i16, ptr %3, align 2, !tbaa !9
  %16 = add i16 %15, 1
  store i16 %16, ptr %3, align 2, !tbaa !9
  br label %4, !llvm.loop !11

17:                                               ; preds = %4
  store i16 7, ptr @src, align 1, !tbaa !13
  store i16 9, ptr getelementptr inbounds nuw (i8, ptr @src, i16 198), align 1, !tbaa !15
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %18

18:                                               ; preds = %27, %17
  %19 = load i16, ptr %3, align 2, !tbaa !9
  %20 = icmp ult i16 %19, 196
  br i1 %20, label %21, label %30

21:                                               ; preds = %18
  %22 = load i16, ptr %3, align 2, !tbaa !9
  %23 = add i16 %22, 3
  %24 = trunc i16 %23 to i8
  %25 = load i16, ptr %3, align 2, !tbaa !9
  %26 = getelementptr inbounds nuw [196 x i8], ptr getelementptr inbounds nuw (i8, ptr @src, i16 2), i16 0, i16 %25
  store i8 %24, ptr %26, align 1, !tbaa !10
  br label %27

27:                                               ; preds = %21
  %28 = load i16, ptr %3, align 2, !tbaa !9
  %29 = add i16 %28, 1
  store i16 %29, ptr %3, align 2, !tbaa !9
  br label %18, !llvm.loop !16

30:                                               ; preds = %18
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 @dst, ptr align 1 @src, i16 200, i1 false), !tbaa.struct !17
  %31 = call i32 @same(ptr noundef @src, ptr noundef @dst, i16 noundef 200) #4
  %32 = load i32, ptr %2, align 2, !tbaa !7
  %33 = add nsw i32 %32, %31
  store i32 %33, ptr %2, align 2, !tbaa !7
  %34 = load i16, ptr @dst, align 1, !tbaa !13
  %35 = icmp ne i16 %34, 7
  br i1 %35, label %39, label %36

36:                                               ; preds = %30
  %37 = load i16, ptr getelementptr inbounds nuw (i8, ptr @dst, i16 198), align 1, !tbaa !15
  %38 = icmp ne i16 %37, 9
  br label %39

39:                                               ; preds = %36, %30
  %40 = phi i1 [ true, %30 ], [ %38, %36 ]
  %41 = zext i1 %40 to i16
  %42 = sext i16 %41 to i32
  %43 = load i32, ptr %2, align 2, !tbaa !7
  %44 = add nsw i32 %43, %42
  store i32 %44, ptr %2, align 2, !tbaa !7
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %45

45:                                               ; preds = %51, %39
  %46 = load i16, ptr %3, align 2, !tbaa !9
  %47 = icmp ult i16 %46, 300
  br i1 %47, label %48, label %54

48:                                               ; preds = %45
  %49 = load i16, ptr %3, align 2, !tbaa !9
  %50 = getelementptr inbounds nuw [300 x i8], ptr @to, i16 0, i16 %49
  store i8 -86, ptr %50, align 1, !tbaa !10
  br label %51

51:                                               ; preds = %48
  %52 = load i16, ptr %3, align 2, !tbaa !9
  %53 = add i16 %52, 1
  store i16 %53, ptr %3, align 2, !tbaa !9
  br label %45, !llvm.loop !18

54:                                               ; preds = %45
  %55 = load i16, ptr @off, align 2, !tbaa !9
  %56 = getelementptr inbounds nuw i8, ptr @to, i16 %55
  %57 = load i16, ptr @off, align 2, !tbaa !9
  %58 = mul i16 2, %57
  %59 = getelementptr inbounds nuw i8, ptr @from, i16 %58
  %60 = load i16, ptr @n_var, align 2, !tbaa !9
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %56, ptr align 1 %59, i16 %60, i1 false)
  %61 = load i8, ptr @to, align 1, !tbaa !10
  %62 = zext i8 %61 to i16
  %63 = icmp ne i16 %62, 170
  br i1 %63, label %72, label %64

64:                                               ; preds = %54
  %65 = load i16, ptr @off, align 2, !tbaa !9
  %66 = load i16, ptr @n_var, align 2, !tbaa !9
  %67 = add i16 %65, %66
  %68 = getelementptr inbounds nuw [300 x i8], ptr @to, i16 0, i16 %67
  %69 = load i8, ptr %68, align 1, !tbaa !10
  %70 = zext i8 %69 to i16
  %71 = icmp ne i16 %70, 170
  br label %72

72:                                               ; preds = %64, %54
  %73 = phi i1 [ true, %54 ], [ %71, %64 ]
  %74 = zext i1 %73 to i16
  %75 = sext i16 %74 to i32
  %76 = load i32, ptr %2, align 2, !tbaa !7
  %77 = add nsw i32 %76, %75
  store i32 %77, ptr %2, align 2, !tbaa !7
  %78 = load i16, ptr @off, align 2, !tbaa !9
  %79 = getelementptr inbounds nuw i8, ptr @to, i16 %78
  %80 = load i16, ptr @off, align 2, !tbaa !9
  %81 = mul i16 2, %80
  %82 = getelementptr inbounds nuw i8, ptr @from, i16 %81
  %83 = load i16, ptr @n_var, align 2, !tbaa !9
  %84 = call i32 @same(ptr noundef %79, ptr noundef %82, i16 noundef %83) #4
  %85 = load i32, ptr %2, align 2, !tbaa !7
  %86 = add nsw i32 %85, %84
  store i32 %86, ptr %2, align 2, !tbaa !7
  %87 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %87) #4
  %88 = load i16, ptr @off, align 2, !tbaa !9
  %89 = load i16, ptr @n_var, align 2, !tbaa !9
  %90 = add i16 %88, %89
  %91 = sub i16 %90, 1
  %92 = getelementptr inbounds nuw [300 x i8], ptr @to, i16 0, i16 %91
  %93 = load i8, ptr %92, align 1, !tbaa !10
  %94 = zext i8 %93 to i32
  call void @report(i32 noundef %94) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: nounwind optsize
define internal i32 @same(ptr noundef %0, ptr noundef %1, i16 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  %6 = alloca ptr, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i32, align 2
  store ptr %0, ptr %5, align 2, !tbaa !19
  store ptr %1, ptr %6, align 2, !tbaa !19
  store i16 %2, ptr %7, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i16 0, ptr %8, align 2, !tbaa !9
  br label %10

10:                                               ; preds = %28, %3
  %11 = load i16, ptr %8, align 2, !tbaa !9
  %12 = load i16, ptr %7, align 2, !tbaa !9
  %13 = icmp ult i16 %11, %12
  br i1 %13, label %14, label %31

14:                                               ; preds = %10
  %15 = load ptr, ptr %5, align 2, !tbaa !19
  %16 = load i16, ptr %8, align 2, !tbaa !9
  %17 = getelementptr inbounds nuw i8, ptr %15, i16 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = zext i8 %18 to i16
  %20 = load ptr, ptr %6, align 2, !tbaa !19
  %21 = load i16, ptr %8, align 2, !tbaa !9
  %22 = getelementptr inbounds nuw i8, ptr %20, i16 %21
  %23 = load i8, ptr %22, align 1, !tbaa !10
  %24 = zext i8 %23 to i16
  %25 = icmp ne i16 %19, %24
  br i1 %25, label %26, label %27

26:                                               ; preds = %14
  store i32 1, ptr %4, align 2
  store i32 1, ptr %9, align 2
  br label %32

27:                                               ; preds = %14
  br label %28

28:                                               ; preds = %27
  %29 = load i16, ptr %8, align 2, !tbaa !9
  %30 = add i16 %29, 1
  store i16 %30, ptr %8, align 2, !tbaa !9
  br label %10, !llvm.loop !22

31:                                               ; preds = %10
  store i32 0, ptr %4, align 2
  store i32 1, ptr %9, align 2
  br label %32

32:                                               ; preds = %31, %26
  call void @llvm.lifetime.end.p0(ptr %8) #3
  %33 = load i32, ptr %4, align 2
  ret i32 %33
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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!5, !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!14, !4, i64 0}
!14 = !{!"big", !4, i64 0, !5, i64 2, !4, i64 198}
!15 = !{!14, !4, i64 198}
!16 = distinct !{!16, !12}
!17 = !{i64 0, i64 2, !9, i64 2, i64 196, !10, i64 198, i64 2, !9}
!18 = distinct !{!18, !12}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 omnipotent char", !21, i64 0}
!21 = !{!"any pointer", !5, i64 0}
!22 = distinct !{!22, !12}
