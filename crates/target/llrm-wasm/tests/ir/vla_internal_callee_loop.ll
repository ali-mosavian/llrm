; ModuleID = 'tests/run/c/vla_loop_stack_restored.c'
source_filename = "tests/run/c/vla_loop_stack_restored.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

@__main_void = hidden alias i32 (), ptr @main

; Function Attrs: nounwind
define hidden i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @nested()
  call void @report(i32 noundef %2)
  ret i32 0
}

declare void @report(i32 noundef) #1

; Function Attrs: noinline nounwind
define internal i32 @nested() #2 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 4
  %6 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %1) #5
  store i32 0, ptr %1, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %2) #5
  store i32 0, ptr %2, align 4, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %3) #5
  call void @llvm.lifetime.start.p0(ptr %4) #5
  store i32 1, ptr %3, align 4, !tbaa !8
  br label %7

7:                                                ; preds = %53, %0
  %8 = load i32, ptr %3, align 4, !tbaa !8
  %9 = icmp slt i32 %8, 7
  br i1 %9, label %10, label %56

10:                                               ; preds = %7
  %11 = load i32, ptr %3, align 4, !tbaa !8
  %12 = mul nsw i32 %11, 3
  %13 = call ptr @llvm.stacksave.p0()
  store ptr %13, ptr %5, align 4
  %14 = alloca i8, i32 %12, align 16
  store i32 %12, ptr %6, align 4
  store i32 0, ptr %4, align 4, !tbaa !8
  br label %15

15:                                               ; preds = %27, %10
  %16 = load i32, ptr %4, align 4, !tbaa !8
  %17 = load i32, ptr %3, align 4, !tbaa !8
  %18 = mul nsw i32 %17, 3
  %19 = icmp slt i32 %16, %18
  br i1 %19, label %20, label %30

20:                                               ; preds = %15
  %21 = load i32, ptr %3, align 4, !tbaa !8
  %22 = load i32, ptr %4, align 4, !tbaa !8
  %23 = add nsw i32 %21, %22
  %24 = trunc i32 %23 to i8
  %25 = load i32, ptr %4, align 4, !tbaa !8
  %26 = getelementptr inbounds i8, ptr %14, i32 %25
  store i8 %24, ptr %26, align 1, !tbaa !9
  br label %27

27:                                               ; preds = %20
  %28 = load i32, ptr %4, align 4, !tbaa !8
  %29 = add nsw i32 %28, 1
  store i32 %29, ptr %4, align 4, !tbaa !8
  br label %15, !llvm.loop !10

30:                                               ; preds = %15
  %31 = load i32, ptr %2, align 4, !tbaa !6
  %32 = icmp ne i32 %31, 0
  br i1 %32, label %35, label %33

33:                                               ; preds = %30
  %34 = ptrtoint ptr %14 to i32
  store i32 %34, ptr %2, align 4, !tbaa !6
  br label %35

35:                                               ; preds = %33, %30
  %36 = load i32, ptr %2, align 4, !tbaa !6
  %37 = ptrtoint ptr %14 to i32
  %38 = sub nsw i32 %36, %37
  %39 = icmp sgt i32 %38, 32
  %40 = zext i1 %39 to i32
  call void @report(i32 noundef %40)
  %41 = load i32, ptr %3, align 4, !tbaa !8
  %42 = mul nsw i32 %41, 3
  %43 = sub nsw i32 %42, 1
  %44 = getelementptr inbounds i8, ptr %14, i32 %43
  %45 = load i8, ptr %44, align 1, !tbaa !9
  %46 = sext i8 %45 to i32
  %47 = load i32, ptr %3, align 4, !tbaa !8
  %48 = call i32 @vla(i32 noundef %47)
  %49 = add nsw i32 %46, %48
  %50 = load i32, ptr %1, align 4, !tbaa !6
  %51 = add nsw i32 %50, %49
  store i32 %51, ptr %1, align 4, !tbaa !6
  %52 = load ptr, ptr %5, align 4
  call void @llvm.stackrestore.p0(ptr %52)
  br label %53

53:                                               ; preds = %35
  %54 = load i32, ptr %3, align 4, !tbaa !8
  %55 = add nsw i32 %54, 1
  store i32 %55, ptr %3, align 4, !tbaa !8
  br label %7, !llvm.loop !12

56:                                               ; preds = %7
  %57 = load i32, ptr %1, align 4, !tbaa !6
  call void @llvm.lifetime.end.p0(ptr %4) #5
  call void @llvm.lifetime.end.p0(ptr %3) #5
  call void @llvm.lifetime.end.p0(ptr %2) #5
  call void @llvm.lifetime.end.p0(ptr %1) #5
  ret i32 %57
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare ptr @llvm.stacksave.p0() #4

; Function Attrs: nounwind
define internal i32 @vla(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca ptr, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !8
  %6 = load i32, ptr %2, align 4, !tbaa !8
  %7 = call ptr @llvm.stacksave.p0()
  store ptr %7, ptr %3, align 4
  %8 = alloca i8, i32 %6, align 16
  store i32 %6, ptr %4, align 4
  call void @llvm.lifetime.start.p0(ptr %5) #5
  store i32 0, ptr %5, align 4, !tbaa !8
  br label %9

9:                                                ; preds = %18, %1
  %10 = load i32, ptr %5, align 4, !tbaa !8
  %11 = load i32, ptr %2, align 4, !tbaa !8
  %12 = icmp slt i32 %10, %11
  br i1 %12, label %13, label %21

13:                                               ; preds = %9
  %14 = load i32, ptr %5, align 4, !tbaa !8
  %15 = trunc i32 %14 to i8
  %16 = load i32, ptr %5, align 4, !tbaa !8
  %17 = getelementptr inbounds i8, ptr %8, i32 %16
  store i8 %15, ptr %17, align 1, !tbaa !9
  br label %18

18:                                               ; preds = %13
  %19 = load i32, ptr %5, align 4, !tbaa !8
  %20 = add nsw i32 %19, 1
  store i32 %20, ptr %5, align 4, !tbaa !8
  br label %9, !llvm.loop !13

21:                                               ; preds = %9
  %22 = load i32, ptr %2, align 4, !tbaa !8
  %23 = sub nsw i32 %22, 1
  %24 = getelementptr inbounds i8, ptr %8, i32 %23
  %25 = load i8, ptr %24, align 1, !tbaa !9
  %26 = sext i8 %25 to i32
  call void @llvm.lifetime.end.p0(ptr %5) #5
  %27 = load ptr, ptr %3, align 4
  call void @llvm.stackrestore.p0(ptr %27)
  ret i32 %26
}

; Function Attrs: nocallback nofree nosync nounwind willreturn
declare void @llvm.stackrestore.p0(ptr) #4

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #3

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #2 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #3 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #4 = { nocallback nofree nosync nounwind willreturn }
attributes #5 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"long", !4, i64 0}
!8 = !{!3, !3, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
