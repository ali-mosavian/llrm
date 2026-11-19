target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = '/home/alim/scratch/wt-align/bench/particle/particle.c'
source_filename = "/home/alim/scratch/wt-align/bench/particle/particle.c"
target triple = "msp430"

@px = internal global [101 x i16] zeroinitializer, align 2
@py = internal global [101 x i16] zeroinitializer, align 2
@pz = internal global [101 x i16] zeroinitializer, align 2
@vx = internal global [101 x i16] zeroinitializer, align 2
@vy = internal global [101 x i16] zeroinitializer, align 2
@vz = internal global [101 x i16] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i32 @bench_particle() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %33, %0
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = icmp sle i16 %6, 100
  br i1 %7, label %8, label %36

8:                                                ; preds = %5
  %9 = load i16, ptr %1, align 2, !tbaa !7
  %10 = sub nsw i16 %9, 50
  store i16 %10, ptr %3, align 2, !tbaa !7
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = load i16, ptr %1, align 2, !tbaa !7
  %13 = getelementptr inbounds [101 x i16], ptr @px, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = mul nsw i16 2, %14
  %16 = load i16, ptr %1, align 2, !tbaa !7
  %17 = getelementptr inbounds [101 x i16], ptr @py, i16 0, i16 %16
  store i16 %15, ptr %17, align 2, !tbaa !7
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = sub nsw i16 0, %18
  %20 = load i16, ptr %1, align 2, !tbaa !7
  %21 = getelementptr inbounds [101 x i16], ptr @pz, i16 0, i16 %20
  store i16 %19, ptr %21, align 2, !tbaa !7
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = and i16 %22, 7
  %24 = load i16, ptr %1, align 2, !tbaa !7
  %25 = getelementptr inbounds [101 x i16], ptr @vx, i16 0, i16 %24
  store i16 %23, ptr %25, align 2, !tbaa !7
  %26 = load i16, ptr %3, align 2, !tbaa !7
  %27 = and i16 %26, 3
  %28 = sub nsw i16 3, %27
  %29 = load i16, ptr %1, align 2, !tbaa !7
  %30 = getelementptr inbounds [101 x i16], ptr @vy, i16 0, i16 %29
  store i16 %28, ptr %30, align 2, !tbaa !7
  %31 = load i16, ptr %1, align 2, !tbaa !7
  %32 = getelementptr inbounds [101 x i16], ptr @vz, i16 0, i16 %31
  store i16 1, ptr %32, align 2, !tbaa !7
  br label %33

33:                                               ; preds = %8
  %34 = load i16, ptr %1, align 2, !tbaa !7
  %35 = add nsw i16 %34, 1
  store i16 %35, ptr %1, align 2, !tbaa !7
  br label %5, !llvm.loop !9

36:                                               ; preds = %5
  store i16 1, ptr %2, align 2, !tbaa !7
  br label %37

37:                                               ; preds = %41, %36
  %38 = load i16, ptr %2, align 2, !tbaa !7
  %39 = icmp sle i16 %38, 100
  br i1 %39, label %40, label %44

40:                                               ; preds = %37
  call void @advance(i16 noundef signext 50) #4
  br label %41

41:                                               ; preds = %40
  %42 = load i16, ptr %2, align 2, !tbaa !7
  %43 = add nsw i16 %42, 1
  store i16 %43, ptr %2, align 2, !tbaa !7
  br label %37, !llvm.loop !11

44:                                               ; preds = %37
  store i32 0, ptr %4, align 2, !tbaa !12
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %45

45:                                               ; preds = %67, %44
  %46 = load i16, ptr %1, align 2, !tbaa !7
  %47 = icmp sle i16 %46, 100
  br i1 %47, label %48, label %70

48:                                               ; preds = %45
  %49 = load i32, ptr %4, align 2, !tbaa !12
  %50 = load i16, ptr %1, align 2, !tbaa !7
  %51 = getelementptr inbounds [101 x i16], ptr @px, i16 0, i16 %50
  %52 = load i16, ptr %51, align 2, !tbaa !7
  %53 = sext i16 %52 to i32
  %54 = add nsw i32 %49, %53
  %55 = load i16, ptr %1, align 2, !tbaa !7
  %56 = getelementptr inbounds [101 x i16], ptr @py, i16 0, i16 %55
  %57 = load i16, ptr %56, align 2, !tbaa !7
  %58 = sext i16 %57 to i32
  %59 = mul nsw i32 3, %58
  %60 = add nsw i32 %54, %59
  %61 = load i16, ptr %1, align 2, !tbaa !7
  %62 = getelementptr inbounds [101 x i16], ptr @pz, i16 0, i16 %61
  %63 = load i16, ptr %62, align 2, !tbaa !7
  %64 = sext i16 %63 to i32
  %65 = mul nsw i32 7, %64
  %66 = add nsw i32 %60, %65
  store i32 %66, ptr %4, align 2, !tbaa !12
  br label %67

67:                                               ; preds = %48
  %68 = load i16, ptr %1, align 2, !tbaa !7
  %69 = add nsw i16 %68, 1
  store i16 %69, ptr %1, align 2, !tbaa !7
  br label %45, !llvm.loop !14

70:                                               ; preds = %45
  %71 = load i32, ptr %4, align 2, !tbaa !12
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %71
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @advance(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = sub nsw i16 50, %4
  store i16 %5, ptr %3, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %39, %1
  %7 = load i16, ptr %3, align 2, !tbaa !7
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = add nsw i16 50, %8
  %10 = icmp sle i16 %7, %9
  br i1 %10, label %11, label %42

11:                                               ; preds = %6
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = getelementptr inbounds [101 x i16], ptr @px, i16 0, i16 %12
  %14 = load i16, ptr %13, align 2, !tbaa !7
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = getelementptr inbounds [101 x i16], ptr @vx, i16 0, i16 %15
  %17 = load i16, ptr %16, align 2, !tbaa !7
  %18 = add nsw i16 %14, %17
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = getelementptr inbounds [101 x i16], ptr @px, i16 0, i16 %19
  store i16 %18, ptr %20, align 2, !tbaa !7
  %21 = load i16, ptr %3, align 2, !tbaa !7
  %22 = getelementptr inbounds [101 x i16], ptr @py, i16 0, i16 %21
  %23 = load i16, ptr %22, align 2, !tbaa !7
  %24 = load i16, ptr %3, align 2, !tbaa !7
  %25 = getelementptr inbounds [101 x i16], ptr @vy, i16 0, i16 %24
  %26 = load i16, ptr %25, align 2, !tbaa !7
  %27 = add nsw i16 %23, %26
  %28 = load i16, ptr %3, align 2, !tbaa !7
  %29 = getelementptr inbounds [101 x i16], ptr @py, i16 0, i16 %28
  store i16 %27, ptr %29, align 2, !tbaa !7
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = getelementptr inbounds [101 x i16], ptr @pz, i16 0, i16 %30
  %32 = load i16, ptr %31, align 2, !tbaa !7
  %33 = load i16, ptr %3, align 2, !tbaa !7
  %34 = getelementptr inbounds [101 x i16], ptr @vz, i16 0, i16 %33
  %35 = load i16, ptr %34, align 2, !tbaa !7
  %36 = add nsw i16 %32, %35
  %37 = load i16, ptr %3, align 2, !tbaa !7
  %38 = getelementptr inbounds [101 x i16], ptr @pz, i16 0, i16 %37
  store i16 %36, ptr %38, align 2, !tbaa !7
  br label %39

39:                                               ; preds = %11
  %40 = load i16, ptr %3, align 2, !tbaa !7
  %41 = add nsw i16 %40, 1
  store i16 %41, ptr %3, align 2, !tbaa !7
  br label %6, !llvm.loop !15

42:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_particle() #4
  call void @report(i32 noundef %2) #4
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

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
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = !{!13, !13, i64 0}
!13 = !{!"long", !5, i64 0}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
