target triple = "msp430"

@big = dso_local constant i64 17179869184, align 2
@most = dso_local global i64 -1, align 2
@neg = dso_local global i64 -5000000000, align 2
@table = dso_local global [3 x i64] [i64 1099511627776, i64 -2, i64 77], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @report(i32 noundef 16) #2
  %2 = load i64, ptr @most, align 2, !tbaa !7
  %3 = lshr i64 %2, 60
  %4 = trunc i64 %3 to i32
  call void @report(i32 noundef %4) #2
  %5 = load i64, ptr @neg, align 2, !tbaa !7
  %6 = sdiv i64 %5, 1000000
  %7 = trunc i64 %6 to i32
  call void @report(i32 noundef %7) #2
  %8 = load i64, ptr @table, align 2, !tbaa !7
  %9 = ashr i64 %8, 38
  %10 = trunc i64 %9 to i32
  %11 = load i64, ptr getelementptr inbounds nuw (i8, ptr @table, i16 8), align 2, !tbaa !7
  %12 = trunc i64 %11 to i32
  %13 = add nsw i32 %10, %12
  %14 = load i64, ptr getelementptr inbounds nuw (i8, ptr @table, i16 16), align 2, !tbaa !7
  %15 = trunc i64 %14 to i32
  %16 = add nsw i32 %13, %15
  call void @report(i32 noundef %16) #2
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { optsize }

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
!8 = !{!"long long", !5, i64 0}
