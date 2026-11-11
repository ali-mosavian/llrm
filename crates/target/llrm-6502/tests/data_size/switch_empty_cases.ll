target triple = "msp430"

@seen = internal global i16 0, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %3

3:                                                ; preds = %8, %0
  %4 = load i16, ptr %2, align 2, !tbaa !7
  %5 = icmp slt i16 %4, 6
  br i1 %5, label %6, label %11

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !7
  call void @classify(i16 noundef %7) #4
  br label %8

8:                                                ; preds = %6
  %9 = load i16, ptr %2, align 2, !tbaa !7
  %10 = add nsw i16 %9, 1
  store i16 %10, ptr %2, align 2, !tbaa !7
  br label %3, !llvm.loop !8

11:                                               ; preds = %3
  %12 = load i16, ptr @seen, align 2, !tbaa !7
  %13 = sext i16 %12 to i32
  call void @report(i32 noundef %13) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @classify(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  switch i16 %3, label %5 [
    i16 1, label %5
    i16 2, label %5
    i16 3, label %4
    i16 4, label %4
  ]

4:                                                ; preds = %1, %1
  br label %5

5:                                                ; preds = %1, %4, %1, %1
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = add nsw i16 %6, 1
  call void @bump(i16 noundef %7) #4
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @bump(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr @seen, align 2, !tbaa !7
  %4 = mul nsw i16 %3, 3
  %5 = load i16, ptr %2, align 2, !tbaa !7
  %6 = add nsw i16 %4, %5
  store i16 %6, ptr @seen, align 2, !tbaa !7
  ret void
}

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
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
