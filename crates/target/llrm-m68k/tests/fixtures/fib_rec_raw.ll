; ModuleID = 'bench/fib/fib.c'
source_filename = "bench/fib/fib.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nofree nosync nounwind memory(none)
define dso_local i32 @bench_fib(i32 noundef %0) local_unnamed_addr #0 {
  %2 = tail call fastcc i32 @fib(i32 noundef %0) #3
  ret i32 %2
}

; Function Attrs: nofree nosync nounwind memory(none)
define internal fastcc i32 @fib(i32 noundef %0) unnamed_addr #0 {
  br label %2

2:                                                ; preds = %6, %1
  %3 = phi i32 [ 0, %1 ], [ %10, %6 ]
  %4 = phi i32 [ %0, %1 ], [ %9, %6 ]
  %5 = icmp slt i32 %4, 2
  br i1 %5, label %11, label %6

6:                                                ; preds = %2
  %7 = add nsw i32 %4, -1
  %8 = tail call fastcc i32 @fib(i32 noundef %7) #3
  %9 = add nsw i32 %4, -2
  %10 = add i32 %3, %8
  br label %2

11:                                               ; preds = %2
  %12 = add i32 %3, %4
  ret i32 %12
}

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #1 {
  %1 = tail call fastcc i32 @fib(i32 noundef 20) #3
  tail call void @report(i32 noundef %1) #4
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #2

attributes #0 = { nofree nosync nounwind memory(none) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nobuiltin "no-builtins" }
attributes #4 = { nobuiltin nounwind "no-builtins" }

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
