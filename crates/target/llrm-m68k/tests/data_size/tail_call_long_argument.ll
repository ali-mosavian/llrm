target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define void @kernel(i32 noundef %0, i32 noundef %1, i32 noundef %2) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %4 = mul nsw i32 %0, 1000
  %5 = add nsw i32 %4, %1
  %6 = add nsw i32 %5, %2
  tail call void @report(i32 noundef %6) nobuiltin "no-builtins"
  ret void
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  tail call void @kernel(i32 noundef 12, i32 noundef 320, i32 noundef 320) nobuiltin "no-builtins"
  tail call void @kernel(i32 noundef 11, i32 noundef -320, i32 noundef -320) nobuiltin "no-builtins"
  ret i32 0
}

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
