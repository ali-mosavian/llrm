target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_fib_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(none) nofree nosync {
  %2 = tail call cc 1002 i32 @fib_(i32 inreg noundef %0)
  ret i32 %2
}

define internal cc 1002 i32 @fib_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(none) nofree nosync {
  %2 = icmp slt i32 %0, 2
  br i1 %2, label %11, label %3

3:
  %4 = phi i32 [ %8, %3 ], [ %0, %1 ]
  %5 = phi i32 [ %9, %3 ], [ 0, %1 ]
  %6 = add nsw i32 %4, -1
  %7 = tail call cc 1002 i32 @fib_(i32 inreg noundef %6)
  %8 = add nsw i32 %4, -2
  %9 = add i32 %7, %5
  %10 = icmp samesign ult i32 %4, 4
  br i1 %10, label %11, label %3

11:
  %12 = phi i32 [ %0, %1 ], [ %8, %3 ]
  %accumulator.tr.lcssa = phi i32 [ 0, %1 ], [ %9, %3 ]
  %accumulator.ret.tr = add i32 %12, %accumulator.tr.lcssa
  ret i32 %accumulator.ret.tr
}

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = tail call cc 1002 i32 @fib_(i32 inreg noundef 20)
  tail call cc 1002 void @report_(i32 inreg noundef %1)
  ret i32 0
}

declare cc 1002 void @report_(i32 inreg noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87"

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
