; ModuleID = 'tests/run/c/tail_call_long_argument.c'
source_filename = "tests/run/c/tail_call_long_argument.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

; Function Attrs: noinline nounwind
define dso_local void @kernel(i16 noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %4, align 2
  store i16 %1, ptr %5, align 2
  store i16 %2, ptr %6, align 2
  %7 = load i16, ptr %4, align 2
  %8 = zext i16 %7 to i32
  %9 = mul nsw i32 %8, 1000
  %10 = load i16, ptr %5, align 2
  %11 = sext i16 %10 to i32
  %12 = add nsw i32 %9, %11
  %13 = load i16, ptr %6, align 2
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %12, %14
  call void @report(i32 noundef %15) #2
  ret void
}

declare dso_local void @report(i32 noundef) #1

; Function Attrs: noinline nounwind
define dso_local i16 @main() #0 {
  call void @kernel(i16 noundef 12, i16 noundef 320, i16 noundef 320) #2
  call void @kernel(i16 noundef 11, i16 noundef -320, i16 noundef -320) #2
  ret i16 0
}

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
