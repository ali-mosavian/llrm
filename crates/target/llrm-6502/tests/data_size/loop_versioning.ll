target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@cells = internal global [256 x i16] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @clear() #3
  call void @fill(ptr noundef @cells, i16 noundef signext 1, i16 noundef signext 40, i16 noundef signext 5) #3
  %2 = call i32 @total(i16 noundef signext 100) #3
  call void @report(i32 noundef %2) #3
  call void @clear() #3
  call void @fill(ptr noundef @cells, i16 noundef signext 2, i16 noundef signext 40, i16 noundef signext 7) #3
  %3 = call i32 @total(i16 noundef signext 100) #3
  call void @report(i32 noundef %3) #3
  call void @clear() #3
  call void @fill(ptr noundef @cells, i16 noundef signext 0, i16 noundef signext 40, i16 noundef signext 9) #3
  %4 = call i32 @total(i16 noundef signext 100) #3
  call void @report(i32 noundef %4) #3
  call void @clear() #3
  call void @fill(ptr noundef @cells, i16 noundef signext 3, i16 noundef signext 0, i16 noundef signext 11) #3
  %5 = call i32 @total(i16 noundef signext 100) #3
  call void @report(i32 noundef %5) #3
  call void @clear() #3
  call void @fill(ptr noundef @cells, i16 noundef signext 3, i16 noundef signext 70, i16 noundef signext 13) #3
  %6 = call i32 @total(i16 noundef signext 250) #3
  call void @report(i32 noundef %6) #3
  ret i16 0
}

; Function Attrs: nounwind optsize
define internal void @clear() #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  store i16 0, ptr %1, align 2, !tbaa !7
  br label %2

2:                                                ; preds = %10, %0
  %3 = load i16, ptr %1, align 2, !tbaa !7
  %4 = icmp slt i16 %3, 256
  br i1 %4, label %5, label %13

5:                                                ; preds = %2
  %6 = load i16, ptr %1, align 2, !tbaa !7
  %7 = add nsw i16 1000, %6
  %8 = load i16, ptr %1, align 2, !tbaa !7
  %9 = getelementptr inbounds [256 x i16], ptr @cells, i16 0, i16 %8
  store i16 %7, ptr %9, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %5
  %11 = load i16, ptr %1, align 2, !tbaa !7
  %12 = add nsw i16 %11, 1
  store i16 %12, ptr %1, align 2, !tbaa !7
  br label %2, !llvm.loop !9

13:                                               ; preds = %2
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret void
}

; Function Attrs: nounwind optsize
define internal void @fill(ptr noundef %0, i16 noundef signext %1, i16 noundef signext %2, i16 noundef signext %3) #0 {
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  store ptr %0, ptr %5, align 2, !tbaa !11
  store i16 %1, ptr %6, align 2, !tbaa !7
  store i16 %2, ptr %7, align 2, !tbaa !7
  store i16 %3, ptr %8, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %9) #4
  store i16 0, ptr %9, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %23, %4
  %11 = load i16, ptr %9, align 2, !tbaa !7
  %12 = load i16, ptr %7, align 2, !tbaa !7
  %13 = icmp slt i16 %11, %12
  br i1 %13, label %14, label %26

14:                                               ; preds = %10
  %15 = load i16, ptr %8, align 2, !tbaa !7
  %16 = load i16, ptr %9, align 2, !tbaa !7
  %17 = add nsw i16 %15, %16
  %18 = load ptr, ptr %5, align 2, !tbaa !11
  %19 = load i16, ptr %9, align 2, !tbaa !7
  %20 = load i16, ptr %6, align 2, !tbaa !7
  %21 = mul nsw i16 %19, %20
  %22 = getelementptr inbounds i16, ptr %18, i16 %21
  store i16 %17, ptr %22, align 2, !tbaa !7
  br label %23

23:                                               ; preds = %14
  %24 = load i16, ptr %9, align 2, !tbaa !7
  %25 = add nsw i16 %24, 1
  store i16 %25, ptr %9, align 2, !tbaa !7
  br label %10, !llvm.loop !14

26:                                               ; preds = %10
  call void @llvm.lifetime.end.p0(ptr %9) #4
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i32 @total(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i32 0, ptr %3, align 2, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %20, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp slt i16 %6, %7
  br i1 %8, label %9, label %23

9:                                                ; preds = %5
  %10 = load i16, ptr %4, align 2, !tbaa !7
  %11 = getelementptr inbounds [256 x i16], ptr @cells, i16 0, i16 %10
  %12 = load i16, ptr %11, align 2, !tbaa !7
  %13 = sext i16 %12 to i32
  %14 = load i16, ptr %4, align 2, !tbaa !7
  %15 = add nsw i16 %14, 1
  %16 = sext i16 %15 to i32
  %17 = mul nsw i32 %13, %16
  %18 = load i32, ptr %3, align 2, !tbaa !15
  %19 = add nsw i32 %18, %17
  store i32 %19, ptr %3, align 2, !tbaa !15
  br label %20

20:                                               ; preds = %9
  %21 = load i16, ptr %4, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !17

23:                                               ; preds = %5
  %24 = load i32, ptr %3, align 2, !tbaa !15
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i32 %24
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
!11 = !{!12, !12, i64 0}
!12 = !{!"p1 short", !13, i64 0}
!13 = !{!"any pointer", !5, i64 0}
!14 = distinct !{!14, !10}
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !5, i64 0}
!17 = distinct !{!17, !10}
