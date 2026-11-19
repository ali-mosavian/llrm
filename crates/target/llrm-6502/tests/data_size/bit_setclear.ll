target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call zeroext i16 @set_bit(i16 noundef zeroext 1056, i16 noundef 6) #3
  %3 = zext i16 %2 to i32
  call void @report(i32 noundef %3) #3
  %4 = call zeroext i16 @set_bit(i16 noundef zeroext 1056, i16 noundef 5) #3
  %5 = zext i16 %4 to i32
  call void @report(i32 noundef %5) #3
  %6 = call zeroext i16 @clear_bit(i16 noundef zeroext 1056, i16 noundef 5) #3
  %7 = zext i16 %6 to i32
  call void @report(i32 noundef %7) #3
  %8 = call zeroext i16 @clear_bit(i16 noundef zeroext 1056, i16 noundef 0) #3
  %9 = zext i16 %8 to i32
  call void @report(i32 noundef %9) #3
  %10 = call i32 @set_long(i32 noundef 0, i16 noundef 31) #3
  call void @report(i32 noundef %10) #3
  %11 = call i32 @clear_long(i32 noundef -1, i16 noundef 0) #3
  call void @report(i32 noundef %11) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: noinline nounwind optsize
define internal zeroext i16 @set_bit(i16 noundef zeroext %0, i16 noundef %1) #2 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = shl i16 1, %6
  %8 = or i16 %5, %7
  ret i16 %8
}

; Function Attrs: noinline nounwind optsize
define internal zeroext i16 @clear_bit(i16 noundef zeroext %0, i16 noundef %1) #2 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = shl i16 1, %6
  %8 = xor i16 %7, -1
  %9 = and i16 %5, %8
  ret i16 %9
}

; Function Attrs: noinline nounwind optsize
define internal i32 @set_long(i32 noundef %0, i16 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i32 %0, ptr %3, align 2, !tbaa !10
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i32, ptr %3, align 2, !tbaa !10
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = zext i16 %6 to i32
  %8 = shl i32 1, %7
  %9 = or i32 %5, %8
  ret i32 %9
}

; Function Attrs: noinline nounwind optsize
define internal i32 @clear_long(i32 noundef %0, i16 noundef %1) #2 {
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  store i32 %0, ptr %3, align 2, !tbaa !10
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i32, ptr %3, align 2, !tbaa !10
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = zext i16 %6 to i32
  %8 = shl i32 1, %7
  %9 = xor i32 %8, -1
  %10 = and i32 %5, %9
  ret i32 %10
}

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
