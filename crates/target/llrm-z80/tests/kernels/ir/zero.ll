source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @zero() #0 {
  ret i8 0
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
