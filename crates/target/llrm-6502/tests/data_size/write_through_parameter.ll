target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca [5 x i16], align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 1, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 2, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @set(ptr noundef %2, i16 noundef 7) #4
  %5 = load i16, ptr %2, align 2, !tbaa !7
  %6 = sext i16 %5 to i32
  call void @report(i32 noundef %6) #4
  call void @add_to(ptr noundef %2, i16 noundef 5) #4
  call void @swap(ptr noundef %2, ptr noundef %3) #4
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = sext i16 %7 to i32
  %9 = mul nsw i32 %8, 100
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = sext i16 %10 to i32
  %12 = add nsw i32 %9, %11
  call void @report(i32 noundef %12) #4
  %13 = getelementptr inbounds [5 x i16], ptr %4, i16 0, i16 0
  call void @fill(ptr noundef %13, i16 noundef 5) #4
  %14 = getelementptr inbounds [5 x i16], ptr %4, i16 0, i16 0
  %15 = call i16 @total(ptr noundef %14, i16 noundef 5) #4
  %16 = sext i16 %15 to i32
  call void @report(i32 noundef %16) #4
  %17 = getelementptr inbounds [5 x i16], ptr %4, i16 0, i16 2
  call void @set(ptr noundef %17, i16 noundef 100) #4
  %18 = getelementptr inbounds [5 x i16], ptr %4, i16 0, i16 0
  %19 = call i16 @total(ptr noundef %18, i16 noundef 5) #4
  %20 = sext i16 %19 to i32
  call void @report(i32 noundef %20) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @set(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %4, align 2, !tbaa !7
  %6 = load ptr, ptr %3, align 2, !tbaa !8
  store i16 %5, ptr %6, align 2, !tbaa !7
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal void @add_to(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %4, align 2, !tbaa !7
  %6 = load ptr, ptr %3, align 2, !tbaa !8
  %7 = load i16, ptr %6, align 2, !tbaa !7
  %8 = add nsw i16 %7, %5
  store i16 %8, ptr %6, align 2, !tbaa !7
  ret void
}

; Function Attrs: nounwind optsize
define internal void @swap(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  %5 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store ptr %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %6 = load ptr, ptr %3, align 2, !tbaa !8
  %7 = load i16, ptr %6, align 2, !tbaa !7
  store i16 %7, ptr %5, align 2, !tbaa !7
  %8 = load ptr, ptr %4, align 2, !tbaa !8
  %9 = load i16, ptr %8, align 2, !tbaa !7
  %10 = load ptr, ptr %3, align 2, !tbaa !8
  store i16 %9, ptr %10, align 2, !tbaa !7
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = load ptr, ptr %4, align 2, !tbaa !8
  store i16 %11, ptr %12, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @fill(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %17, %2
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = load i16, ptr %4, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %20

10:                                               ; preds = %6
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = mul nsw i16 %11, 3
  %13 = add nsw i16 %12, 1
  %14 = load ptr, ptr %3, align 2, !tbaa !8
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = getelementptr inbounds i16, ptr %14, i16 %15
  store i16 %13, ptr %16, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %10
  %18 = load i16, ptr %5, align 2, !tbaa !7
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !11

20:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal i16 @total(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !8
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %18, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %4, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %21

11:                                               ; preds = %7
  %12 = load ptr, ptr %3, align 2, !tbaa !8
  %13 = load i16, ptr %6, align 2, !tbaa !7
  %14 = getelementptr inbounds i16, ptr %12, i16 %13
  %15 = load i16, ptr %14, align 2, !tbaa !7
  %16 = load i16, ptr %5, align 2, !tbaa !7
  %17 = add nsw i16 %16, %15
  store i16 %17, ptr %5, align 2, !tbaa !7
  br label %18

18:                                               ; preds = %11
  %19 = load i16, ptr %6, align 2, !tbaa !7
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !13

21:                                               ; preds = %7
  %22 = load i16, ptr %5, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i16 %22
}

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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 int", !10, i64 0}
!10 = !{!"any pointer", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
