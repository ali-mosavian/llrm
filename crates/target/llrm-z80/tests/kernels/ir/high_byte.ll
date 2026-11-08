target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call zeroext i8 @high(i16 noundef zeroext 4660)
  %3 = zext i8 %2 to i32
  call void @report(i32 noundef %3)
  %4 = call zeroext i8 @high(i16 noundef zeroext -255)
  %5 = zext i8 %4 to i32
  call void @report(i32 noundef %5)
  %6 = call i32 @widen(i16 noundef zeroext -21760)
  call void @report(i32 noundef %6)
  %7 = call i32 @signed_high(i16 noundef signext -32767)
  call void @report(i32 noundef %7)
  %8 = call i32 @signed_high(i16 noundef signext 32512)
  call void @report(i32 noundef %8)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #1

; Function Attrs: noinline nounwind
define internal zeroext i8 @high(i16 noundef zeroext %0) #2 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = lshr i16 %3, 8
  %5 = trunc i16 %4 to i8
  ret i8 %5
}

; Function Attrs: noinline nounwind
define internal i32 @widen(i16 noundef zeroext %0) #2 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = lshr i16 %3, 8
  %5 = trunc i16 %4 to i8
  %6 = zext i8 %5 to i32
  %7 = mul i32 %6, 3
  ret i32 %7
}

; Function Attrs: noinline nounwind
define internal i32 @signed_high(i16 noundef signext %0) #2 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = ashr i16 %3, 8
  %5 = trunc i16 %4 to i8
  %6 = sext i8 %5 to i32
  ret i32 %6
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

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
