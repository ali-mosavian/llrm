target triple = "msp430"

@chosen = internal global ptr @twice, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store i32 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %4

4:                                                ; preds = %11, %0
  %5 = load i16, ptr %3, align 2, !tbaa !9
  %6 = icmp slt i16 %5, 6
  br i1 %6, label %7, label %14

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2, !tbaa !7
  %9 = load i16, ptr %3, align 2, !tbaa !9
  %10 = call i32 @fold(i32 noundef %8, i16 noundef %9) #5
  store i32 %10, ptr %2, align 2, !tbaa !7
  br label %11

11:                                               ; preds = %7
  %12 = load i16, ptr %3, align 2, !tbaa !9
  %13 = add nsw i16 %12, 1
  store i16 %13, ptr %3, align 2, !tbaa !9
  br label %4, !llvm.loop !10

14:                                               ; preds = %4
  %15 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %15) #5
  %16 = call zeroext i16 @low_half(i32 noundef 305419896) #5
  %17 = zext i16 %16 to i32
  call void @report(i32 noundef %17) #5
  %18 = load ptr, ptr @chosen, align 2, !tbaa !12
  %19 = call i16 %18(i16 noundef 21) #5
  %20 = sext i16 %19 to i32
  call void @report(i32 noundef %20) #5
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: inlinehint nounwind optsize
define internal i32 @fold(i32 noundef %0, i16 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = mul nsw i32 %5, 3
  %7 = load i16, ptr %4, align 2, !tbaa !9
  %8 = call i16 @twice(i16 noundef %7) #5
  %9 = sext i16 %8 to i32
  %10 = add nsw i32 %6, %9
  ret i32 %10
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: inlinehint nounwind optsize
define internal zeroext i16 @low_half(i32 noundef %0) #2 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  %3 = load i32, ptr %2, align 2, !tbaa !7
  %4 = trunc i32 %3 to i16
  ret i16 %4
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: inlinehint nounwind optsize
define internal i16 @twice(i16 noundef %0) #2 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !9
  %3 = load i16, ptr %2, align 2, !tbaa !9
  %4 = load i16, ptr %2, align 2, !tbaa !9
  %5 = add nsw i16 %3, %4
  ret i16 %5
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { inlinehint nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

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
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!13, !13, i64 0}
!13 = !{!"any pointer", !5, i64 0}
