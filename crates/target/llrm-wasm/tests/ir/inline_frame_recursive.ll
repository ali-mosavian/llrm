; ModuleID = 'rec.c'
source_filename = "rec.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

; Function Attrs: nounwind
define hidden i32 @entry(ptr noundef %0) #0 {
  %2 = alloca ptr, align 4
  store ptr %0, ptr %2, align 4, !tbaa !6
  %3 = load ptr, ptr %2, align 4, !tbaa !6
  %4 = call i32 @g(i32 noundef 0, ptr noundef %3) #2
  %5 = load ptr, ptr %2, align 4, !tbaa !6
  %6 = call i32 @f(i32 noundef 2, ptr noundef %5) #2
  %7 = add nsw i32 %4, %6
  ret i32 %7
}

; Function Attrs: nounwind
define internal i32 @g(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store ptr %1, ptr %5, align 4, !tbaa !6
  %6 = load i32, ptr %4, align 4, !tbaa !9
  %7 = icmp sgt i32 %6, 3
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i32, ptr %4, align 4, !tbaa !9
  store i32 %9, ptr %3, align 4
  br label %19

10:                                               ; preds = %2
  %11 = load i32, ptr %4, align 4, !tbaa !9
  %12 = load ptr, ptr %5, align 4, !tbaa !6
  %13 = call i32 @f(i32 noundef %11, ptr noundef %12) #2
  %14 = load i32, ptr %4, align 4, !tbaa !9
  %15 = add nsw i32 %14, 1
  %16 = load ptr, ptr %5, align 4, !tbaa !6
  %17 = call i32 @g(i32 noundef %15, ptr noundef %16) #2
  %18 = add nsw i32 %13, %17
  store i32 %18, ptr %3, align 4
  br label %19

19:                                               ; preds = %10, %8
  %20 = load i32, ptr %3, align 4
  ret i32 %20
}

; Function Attrs: nounwind
define internal i32 @f(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 4
  %6 = alloca [16 x i32], align 16
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !9
  store ptr %1, ptr %5, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i32 0, ptr %8, align 4, !tbaa !9
  %10 = load i32, ptr %4, align 4, !tbaa !9
  %11 = icmp eq i32 %10, 0
  br i1 %11, label %12, label %13

12:                                               ; preds = %2
  store i32 7, ptr %3, align 4
  store i32 1, ptr %9, align 4
  br label %46

13:                                               ; preds = %2
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %14

14:                                               ; preds = %23, %13
  %15 = load i32, ptr %7, align 4, !tbaa !9
  %16 = icmp slt i32 %15, 16
  br i1 %16, label %17, label %26

17:                                               ; preds = %14
  %18 = load i32, ptr %4, align 4, !tbaa !9
  %19 = load i32, ptr %7, align 4, !tbaa !9
  %20 = mul nsw i32 %18, %19
  %21 = load i32, ptr %7, align 4, !tbaa !9
  %22 = getelementptr inbounds [16 x i32], ptr %6, i32 0, i32 %21
  store i32 %20, ptr %22, align 4, !tbaa !9
  br label %23

23:                                               ; preds = %17
  %24 = load i32, ptr %7, align 4, !tbaa !9
  %25 = add nsw i32 %24, 1
  store i32 %25, ptr %7, align 4, !tbaa !9
  br label %14, !llvm.loop !11

26:                                               ; preds = %14
  %27 = load ptr, ptr %5, align 4, !tbaa !6
  %28 = load i32, ptr %27, align 4, !tbaa !9
  %29 = load i32, ptr %4, align 4, !tbaa !9
  %30 = add nsw i32 %28, %29
  %31 = getelementptr inbounds [16 x i32], ptr %6, i32 0, i32 3
  store i32 %30, ptr %31, align 4, !tbaa !9
  store i32 0, ptr %7, align 4, !tbaa !9
  br label %32

32:                                               ; preds = %41, %26
  %33 = load i32, ptr %7, align 4, !tbaa !9
  %34 = icmp slt i32 %33, 16
  br i1 %34, label %35, label %44

35:                                               ; preds = %32
  %36 = load i32, ptr %7, align 4, !tbaa !9
  %37 = getelementptr inbounds [16 x i32], ptr %6, i32 0, i32 %36
  %38 = load i32, ptr %37, align 4, !tbaa !9
  %39 = load i32, ptr %8, align 4, !tbaa !9
  %40 = add nsw i32 %39, %38
  store i32 %40, ptr %8, align 4, !tbaa !9
  br label %41

41:                                               ; preds = %35
  %42 = load i32, ptr %7, align 4, !tbaa !9
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %7, align 4, !tbaa !9
  br label %32, !llvm.loop !14

44:                                               ; preds = %32
  %45 = load i32, ptr %8, align 4, !tbaa !9
  store i32 %45, ptr %3, align 4
  store i32 1, ptr %9, align 4
  br label %46

46:                                               ; preds = %44, %12
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %47 = load i32, ptr %3, align 4
  ret i32 %47
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nobuiltin "no-builtins" }
attributes #3 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"p1 long", !8, i64 0}
!8 = !{!"any pointer", !4, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !4, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
