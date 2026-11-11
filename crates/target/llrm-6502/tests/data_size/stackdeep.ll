target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @deep(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca [48 x i8], align 1
  %5 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %6 = load i16, ptr %3, align 2, !tbaa !7
  %7 = trunc i16 %6 to i8
  %8 = getelementptr inbounds [48 x i8], ptr %4, i16 0, i16 0
  store volatile i8 %7, ptr %8, align 1, !tbaa !8
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = icmp eq i16 %9, 0
  br i1 %10, label %11, label %12

11:                                               ; preds = %1
  store i16 0, ptr %2, align 2
  store i32 1, ptr %5, align 2
  br label %25

12:                                               ; preds = %1
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = sub nsw i16 %13, 1
  %15 = call i16 @deep(i16 noundef %14) #4
  %16 = add nsw i16 1, %15
  %17 = getelementptr inbounds [48 x i8], ptr %4, i16 0, i16 0
  %18 = load volatile i8, ptr %17, align 1, !tbaa !8
  %19 = zext i8 %18 to i16
  %20 = add nsw i16 %16, %19
  %21 = load i16, ptr %3, align 2, !tbaa !7
  %22 = trunc i16 %21 to i8
  %23 = zext i8 %22 to i16
  %24 = sub nsw i16 %20, %23
  store i16 %24, ptr %2, align 2
  store i32 1, ptr %5, align 2
  br label %25

25:                                               ; preds = %12, %11
  call void @llvm.lifetime.end.p0(ptr %4) #3
  %26 = load i16, ptr %2, align 2
  ret i16 %26
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i16 @deep(i16 noundef 100) #4
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #4
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
!7 = !{!4, !4, i64 0}
!8 = !{!5, !5, i64 0}
