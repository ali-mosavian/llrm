target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @report(i32 noundef 7) #3
  %2 = call i16 @pick(i16 noundef 0) #3
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #3
  %4 = call i16 @pick(i16 noundef 5) #3
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i16 @pick(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  %4 = load i16, ptr %3, align 2, !tbaa !7
  %5 = sext i16 %4 to i32
  %6 = call i32 @llvm.expect.i32(i32 %5, i32 0)
  %7 = icmp ne i32 %6, 0
  br i1 %7, label %8, label %9

8:                                                ; preds = %1
  store i16 1, ptr %2, align 2
  br label %10

9:                                                ; preds = %1
  store i16 2, ptr %2, align 2
  br label %10

10:                                               ; preds = %9, %8
  %11 = load i16, ptr %2, align 2
  ret i16 %11
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(none)
declare i32 @llvm.expect.i32(i32, i32) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(none) }
attributes #3 = { optsize }

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
