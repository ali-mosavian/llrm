; ModuleID = 'histo.c'
source_filename = "histo.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

@data = internal global [4096 x i8] zeroinitializer, align 1
@counts = internal global [256 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i32 @bench_histo(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !9
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %17, %1
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 4096
  br i1 %7, label %8, label %20

8:                                                ; preds = %5
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = and i16 %9, 127
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = ashr i16 %11, 5
  %13 = mul nsw i16 %10, %12
  %14 = trunc i16 %13 to i8
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = getelementptr inbounds [4096 x i8], ptr @data, i16 0, i16 %15
  store i8 %14, ptr %16, align 1, !tbaa !11
  br label %17

17:                                               ; preds = %8
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %3, align 2, !tbaa !7
  br label %5, !llvm.loop !12

20:                                               ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %21

21:                                               ; preds = %27, %20
  %22 = load i16, ptr %3, align 2, !tbaa !7
  %23 = icmp slt i16 %22, 256
  br i1 %23, label %24, label %30

24:                                               ; preds = %21
  %25 = load i16, ptr %3, align 2, !tbaa !7
  %26 = getelementptr inbounds [256 x i16], ptr @counts, i16 0, i16 %25
  store i16 0, ptr %26, align 2, !tbaa !7
  br label %27

27:                                               ; preds = %24
  %28 = load i16, ptr %3, align 2, !tbaa !7
  %29 = add nsw i16 %28, 1
  store i16 %29, ptr %3, align 2, !tbaa !7
  br label %21, !llvm.loop !15

30:                                               ; preds = %21
  call void @histogram(ptr noundef %2)
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %45, %30
  %32 = load i16, ptr %3, align 2, !tbaa !7
  %33 = icmp slt i16 %32, 256
  br i1 %33, label %34, label %48

34:                                               ; preds = %31
  %35 = load i16, ptr %3, align 2, !tbaa !7
  %36 = getelementptr inbounds [256 x i16], ptr @counts, i16 0, i16 %35
  %37 = load i16, ptr %36, align 2, !tbaa !7
  %38 = sext i16 %37 to i32
  %39 = load i16, ptr %3, align 2, !tbaa !7
  %40 = add nsw i16 %39, 1
  %41 = sext i16 %40 to i32
  %42 = mul nsw i32 %38, %41
  %43 = load i32, ptr %4, align 2, !tbaa !9
  %44 = add nsw i32 %43, %42
  store i32 %44, ptr %4, align 2, !tbaa !9
  br label %45

45:                                               ; preds = %34
  %46 = load i16, ptr %3, align 2, !tbaa !7
  %47 = add nsw i16 %46, 1
  store i16 %47, ptr %3, align 2, !tbaa !7
  br label %31, !llvm.loop !16

48:                                               ; preds = %31
  %49 = load i32, ptr %4, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %49
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal void @histogram(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %20, %1
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = icmp slt i16 %5, 4096
  br i1 %6, label %7, label %23

7:                                                ; preds = %4
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = getelementptr inbounds [4096 x i8], ptr @data, i16 0, i16 %8
  %10 = load i8, ptr %9, align 1, !tbaa !11
  %11 = zext i8 %10 to i16
  %12 = mul nsw i16 %11, 7
  %13 = load ptr, ptr %2, align 2, !tbaa !17
  %14 = load i16, ptr %13, align 2, !tbaa !7
  %15 = add nsw i16 %12, %14
  %16 = and i16 %15, 255
  %17 = getelementptr inbounds [256 x i16], ptr @counts, i16 0, i16 %16
  %18 = load i16, ptr %17, align 2, !tbaa !7
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %17, align 2, !tbaa !7
  br label %20

20:                                               ; preds = %7
  %21 = load i16, ptr %3, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %3, align 2, !tbaa !7
  br label %4, !llvm.loop !20

23:                                               ; preds = %4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_histo(i16 noundef signext 3)
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0}
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
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13, !14}
!13 = !{!"llvm.loop.mustprogress"}
!14 = !{!"llvm.loop.unroll.disable"}
!15 = distinct !{!15, !13, !14}
!16 = distinct !{!16, !13, !14}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 short", !19, i64 0}
!19 = !{!"any pointer", !5, i64 0}
!20 = distinct !{!20, !13, !14}
