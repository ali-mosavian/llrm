source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @lz4(ptr noundef %0, ptr noundef %1, i16 noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i16, align 2
  %10 = alloca ptr, align 2
  %11 = alloca ptr, align 2
  %12 = alloca ptr, align 2
  store ptr %0, ptr %4, align 2, !tbaa !2
  store ptr %1, ptr %5, align 2, !tbaa !2
  store i16 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.start.p0(i64 2, ptr %9) #2
  call void @llvm.lifetime.start.p0(i64 2, ptr %10) #2
  %13 = load ptr, ptr %5, align 2, !tbaa !2
  store ptr %13, ptr %10, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %11) #2
  %14 = load ptr, ptr %5, align 2, !tbaa !2
  %15 = load i16, ptr %6, align 2, !tbaa !7
  %16 = getelementptr inbounds nuw i8, ptr %14, i16 %15
  store ptr %16, ptr %11, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %12) #2
  br label %17

17:                                               ; preds = %117, %3
  %18 = load ptr, ptr %5, align 2, !tbaa !2
  %19 = load ptr, ptr %11, align 2, !tbaa !2
  %20 = icmp ult ptr %18, %19
  br i1 %20, label %21, label %118

21:                                               ; preds = %17
  %22 = load ptr, ptr %4, align 2, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %22, i32 1
  store ptr %23, ptr %4, align 2, !tbaa !2
  %24 = load i8, ptr %22, align 1, !tbaa !9
  store i8 %24, ptr %7, align 1, !tbaa !9
  %25 = load i8, ptr %7, align 1, !tbaa !9
  %26 = zext i8 %25 to i16
  %27 = ashr i16 %26, 4
  store i16 %27, ptr %9, align 2, !tbaa !7
  %28 = load i8, ptr %7, align 1, !tbaa !9
  %29 = zext i8 %28 to i16
  %30 = and i16 %29, 15
  %31 = trunc i16 %30 to i8
  store i8 %31, ptr %7, align 1, !tbaa !9
  %32 = load i8, ptr %7, align 1, !tbaa !9
  %33 = zext i8 %32 to i16
  %34 = add nsw i16 %33, 4
  %35 = trunc i16 %34 to i8
  store i8 %35, ptr %7, align 1, !tbaa !9
  %36 = load i16, ptr %9, align 2, !tbaa !7
  %37 = icmp eq i16 %36, 15
  br i1 %37, label %38, label %52

38:                                               ; preds = %21
  br label %39

39:                                               ; preds = %47, %38
  %40 = load ptr, ptr %4, align 2, !tbaa !2
  %41 = getelementptr inbounds nuw i8, ptr %40, i32 1
  store ptr %41, ptr %4, align 2, !tbaa !2
  %42 = load i8, ptr %40, align 1, !tbaa !9
  store i8 %42, ptr %8, align 1, !tbaa !9
  %43 = load i8, ptr %8, align 1, !tbaa !9
  %44 = zext i8 %43 to i16
  %45 = load i16, ptr %9, align 2, !tbaa !7
  %46 = add i16 %45, %44
  store i16 %46, ptr %9, align 2, !tbaa !7
  br label %47

47:                                               ; preds = %39
  %48 = load i8, ptr %8, align 1, !tbaa !9
  %49 = zext i8 %48 to i16
  %50 = icmp eq i16 %49, 255
  br i1 %50, label %39, label %51, !llvm.loop !10

51:                                               ; preds = %47
  br label %52

52:                                               ; preds = %51, %21
  br label %53

53:                                               ; preds = %56, %52
  %54 = load i16, ptr %9, align 2, !tbaa !7
  %55 = icmp ne i16 %54, 0
  br i1 %55, label %56, label %64

56:                                               ; preds = %53
  %57 = load ptr, ptr %4, align 2, !tbaa !2
  %58 = getelementptr inbounds nuw i8, ptr %57, i32 1
  store ptr %58, ptr %4, align 2, !tbaa !2
  %59 = load i8, ptr %57, align 1, !tbaa !9
  %60 = load ptr, ptr %5, align 2, !tbaa !2
  %61 = getelementptr inbounds nuw i8, ptr %60, i32 1
  store ptr %61, ptr %5, align 2, !tbaa !2
  store i8 %59, ptr %60, align 1, !tbaa !9
  %62 = load i16, ptr %9, align 2, !tbaa !7
  %63 = add i16 %62, -1
  store i16 %63, ptr %9, align 2, !tbaa !7
  br label %53, !llvm.loop !13

64:                                               ; preds = %53
  %65 = load ptr, ptr %5, align 2, !tbaa !2
  %66 = load ptr, ptr %11, align 2, !tbaa !2
  %67 = icmp uge ptr %65, %66
  br i1 %67, label %68, label %69

68:                                               ; preds = %64
  br label %118

69:                                               ; preds = %64
  %70 = load ptr, ptr %4, align 2, !tbaa !2
  %71 = getelementptr inbounds nuw i8, ptr %70, i32 1
  store ptr %71, ptr %4, align 2, !tbaa !2
  %72 = load i8, ptr %70, align 1, !tbaa !9
  %73 = zext i8 %72 to i16
  store i16 %73, ptr %9, align 2, !tbaa !7
  %74 = load ptr, ptr %4, align 2, !tbaa !2
  %75 = getelementptr inbounds nuw i8, ptr %74, i32 1
  store ptr %75, ptr %4, align 2, !tbaa !2
  %76 = load i8, ptr %74, align 1, !tbaa !9
  store i8 %76, ptr %8, align 1, !tbaa !9
  %77 = load i8, ptr %8, align 1, !tbaa !9
  %78 = zext i8 %77 to i16
  %79 = shl i16 %78, 8
  %80 = load i16, ptr %9, align 2, !tbaa !7
  %81 = add i16 %80, %79
  store i16 %81, ptr %9, align 2, !tbaa !7
  %82 = load ptr, ptr %5, align 2, !tbaa !2
  %83 = load i16, ptr %9, align 2, !tbaa !7
  %84 = sub i16 0, %83
  %85 = getelementptr inbounds i8, ptr %82, i16 %84
  store ptr %85, ptr %12, align 2, !tbaa !2
  %86 = load i8, ptr %7, align 1, !tbaa !9
  %87 = zext i8 %86 to i16
  store i16 %87, ptr %9, align 2, !tbaa !7
  %88 = load i8, ptr %7, align 1, !tbaa !9
  %89 = zext i8 %88 to i16
  %90 = icmp eq i16 %89, 19
  br i1 %90, label %91, label %105

91:                                               ; preds = %69
  br label %92

92:                                               ; preds = %100, %91
  %93 = load ptr, ptr %4, align 2, !tbaa !2
  %94 = getelementptr inbounds nuw i8, ptr %93, i32 1
  store ptr %94, ptr %4, align 2, !tbaa !2
  %95 = load i8, ptr %93, align 1, !tbaa !9
  store i8 %95, ptr %8, align 1, !tbaa !9
  %96 = load i8, ptr %8, align 1, !tbaa !9
  %97 = zext i8 %96 to i16
  %98 = load i16, ptr %9, align 2, !tbaa !7
  %99 = add i16 %98, %97
  store i16 %99, ptr %9, align 2, !tbaa !7
  br label %100

100:                                              ; preds = %92
  %101 = load i8, ptr %8, align 1, !tbaa !9
  %102 = zext i8 %101 to i16
  %103 = icmp eq i16 %102, 255
  br i1 %103, label %92, label %104, !llvm.loop !14

104:                                              ; preds = %100
  br label %105

105:                                              ; preds = %104, %69
  br label %106

106:                                              ; preds = %109, %105
  %107 = load i16, ptr %9, align 2, !tbaa !7
  %108 = icmp ne i16 %107, 0
  br i1 %108, label %109, label %117

109:                                              ; preds = %106
  %110 = load ptr, ptr %12, align 2, !tbaa !2
  %111 = getelementptr inbounds nuw i8, ptr %110, i32 1
  store ptr %111, ptr %12, align 2, !tbaa !2
  %112 = load i8, ptr %110, align 1, !tbaa !9
  %113 = load ptr, ptr %5, align 2, !tbaa !2
  %114 = getelementptr inbounds nuw i8, ptr %113, i32 1
  store ptr %114, ptr %5, align 2, !tbaa !2
  store i8 %112, ptr %113, align 1, !tbaa !9
  %115 = load i16, ptr %9, align 2, !tbaa !7
  %116 = add i16 %115, -1
  store i16 %116, ptr %9, align 2, !tbaa !7
  br label %106, !llvm.loop !15

117:                                              ; preds = %106
  br label %17, !llvm.loop !16

118:                                              ; preds = %68, %17
  %119 = load ptr, ptr %5, align 2, !tbaa !2
  %120 = load ptr, ptr %10, align 2, !tbaa !2
  %121 = ptrtoint ptr %119 to i16
  %122 = ptrtoint ptr %120 to i16
  %123 = sub i16 %121, %122
  call void @llvm.lifetime.end.p0(i64 2, ptr %12) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %11) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %10) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %9) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  ret i16 %123
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"p1 omnipotent char", !4, i64 0}
!4 = !{!"any pointer", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"int", !5, i64 0}
!9 = !{!5, !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = distinct !{!13, !11, !12}
!14 = distinct !{!14, !11, !12}
!15 = distinct !{!15, !11, !12}
!16 = distinct !{!16, !11, !12}
