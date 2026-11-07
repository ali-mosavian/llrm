; ModuleID = '/home/alim/scratch/wt-z80f32b/tests/run/c/sibling_call_callees.c'
source_filename = "/home/alim/scratch/wt-z80f32b/tests/run/c/sibling_call_callees.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @g(i16 noundef 1)
  call void @g(i16 noundef 2)
  call void @g(i16 noundef 3)
  ret i16 0
}

; Function Attrs: noinline nounwind
define internal void @g(i16 noundef %0) #1 {
  %2 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  %3 = load i16, ptr %2, align 2, !tbaa !7
  %4 = icmp eq i16 %3, 1
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  call void @f()
  br label %11

6:                                                ; preds = %1
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp eq i16 %7, 2
  br i1 %8, label %9, label %10

9:                                                ; preds = %6
  call void @h()
  br label %10

10:                                               ; preds = %9, %6
  br label %11

11:                                               ; preds = %10, %5
  ret void
}

; Function Attrs: noinline nounwind
define internal void @f() #1 {
  call void @report(i32 noundef 1)
  ret void
}

; Function Attrs: noinline nounwind
define internal void @h() #1 {
  call void @report(i32 noundef 2)
  ret void
}

declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

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
