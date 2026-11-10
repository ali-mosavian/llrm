; ModuleID = 'partition.c'
source_filename = "partition.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @k(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  store ptr %0, ptr %3, align 4, !tbaa !4
  store i32 %1, ptr %4, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(i64 4, ptr %5) #2
  %10 = load ptr, ptr %3, align 4, !tbaa !4
  %11 = load i32, ptr %4, align 4, !tbaa !9
  %12 = sub nsw i32 %11, 1
  %13 = getelementptr inbounds i32, ptr %10, i32 %12
  %14 = load i32, ptr %13, align 4, !tbaa !9
  store i32 %14, ptr %5, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(i64 4, ptr %6) #2
  store i32 0, ptr %6, align 4, !tbaa !9
  call void @llvm.lifetime.start.p0(i64 4, ptr %7) #2
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %15

15:                                               ; preds = %47, %2
  %16 = load i32, ptr %7, align 4, !tbaa !9
  %17 = load i32, ptr %4, align 4, !tbaa !9
  %18 = sub nsw i32 %17, 1
  %19 = icmp slt i32 %16, %18
  br i1 %19, label %21, label %20

20:                                               ; preds = %15
  call void @llvm.lifetime.end.p0(i64 4, ptr %7) #2
  br label %50

21:                                               ; preds = %15
  %22 = load ptr, ptr %3, align 4, !tbaa !4
  %23 = load i32, ptr %7, align 4, !tbaa !9
  %24 = getelementptr inbounds i32, ptr %22, i32 %23
  %25 = load i32, ptr %24, align 4, !tbaa !9
  %26 = load i32, ptr %5, align 4, !tbaa !9
  %27 = icmp slt i32 %25, %26
  br i1 %27, label %28, label %46

28:                                               ; preds = %21
  call void @llvm.lifetime.start.p0(i64 4, ptr %8) #2
  %29 = load ptr, ptr %3, align 4, !tbaa !4
  %30 = load i32, ptr %6, align 4, !tbaa !9
  %31 = getelementptr inbounds i32, ptr %29, i32 %30
  %32 = load i32, ptr %31, align 4, !tbaa !9
  store i32 %32, ptr %8, align 4, !tbaa !9
  %33 = load ptr, ptr %3, align 4, !tbaa !4
  %34 = load i32, ptr %7, align 4, !tbaa !9
  %35 = getelementptr inbounds i32, ptr %33, i32 %34
  %36 = load i32, ptr %35, align 4, !tbaa !9
  %37 = load ptr, ptr %3, align 4, !tbaa !4
  %38 = load i32, ptr %6, align 4, !tbaa !9
  %39 = getelementptr inbounds i32, ptr %37, i32 %38
  store i32 %36, ptr %39, align 4, !tbaa !9
  %40 = load i32, ptr %8, align 4, !tbaa !9
  %41 = load ptr, ptr %3, align 4, !tbaa !4
  %42 = load i32, ptr %7, align 4, !tbaa !9
  %43 = getelementptr inbounds i32, ptr %41, i32 %42
  store i32 %40, ptr %43, align 4, !tbaa !9
  %44 = load i32, ptr %6, align 4, !tbaa !9
  %45 = add nsw i32 %44, 1
  store i32 %45, ptr %6, align 4, !tbaa !9
  call void @llvm.lifetime.end.p0(i64 4, ptr %8) #2
  br label %46

46:                                               ; preds = %28, %21
  br label %47

47:                                               ; preds = %46
  %48 = load i32, ptr %7, align 4, !tbaa !9
  %49 = add nsw i32 %48, 1
  store i32 %49, ptr %7, align 4, !tbaa !9
  br label %15, !llvm.loop !11

50:                                               ; preds = %20
  call void @llvm.lifetime.start.p0(i64 4, ptr %9) #2
  %51 = load ptr, ptr %3, align 4, !tbaa !4
  %52 = load i32, ptr %6, align 4, !tbaa !9
  %53 = getelementptr inbounds i32, ptr %51, i32 %52
  %54 = load i32, ptr %53, align 4, !tbaa !9
  store i32 %54, ptr %9, align 4, !tbaa !9
  %55 = load ptr, ptr %3, align 4, !tbaa !4
  %56 = load i32, ptr %4, align 4, !tbaa !9
  %57 = sub nsw i32 %56, 1
  %58 = getelementptr inbounds i32, ptr %55, i32 %57
  %59 = load i32, ptr %58, align 4, !tbaa !9
  %60 = load ptr, ptr %3, align 4, !tbaa !4
  %61 = load i32, ptr %6, align 4, !tbaa !9
  %62 = getelementptr inbounds i32, ptr %60, i32 %61
  store i32 %59, ptr %62, align 4, !tbaa !9
  %63 = load i32, ptr %9, align 4, !tbaa !9
  %64 = load ptr, ptr %3, align 4, !tbaa !4
  %65 = load i32, ptr %4, align 4, !tbaa !9
  %66 = sub nsw i32 %65, 1
  %67 = getelementptr inbounds i32, ptr %64, i32 %66
  store i32 %63, ptr %67, align 4, !tbaa !9
  %68 = load i32, ptr %6, align 4, !tbaa !9
  %69 = load ptr, ptr %3, align 4, !tbaa !4
  %70 = getelementptr inbounds i32, ptr %69, i32 0
  %71 = load i32, ptr %70, align 4, !tbaa !9
  %72 = mul nsw i32 %71, 16
  %73 = add nsw i32 %68, %72
  call void @llvm.lifetime.end.p0(i64 4, ptr %9) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 4, ptr %5) #2
  ret i32 %73
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!4 = !{!5, !5, i64 0}
!5 = !{!"p1 int", !6, i64 0}
!6 = !{!"any pointer", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"int", !7, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
