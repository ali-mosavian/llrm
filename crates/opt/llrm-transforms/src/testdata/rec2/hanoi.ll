target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_hanoi_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(none) nofree nosync {
  %2 = tail call cc 1002 i32 @hanoi_(i32 inreg noundef %0)
  ret i32 %2
}

define internal cc 1002 i32 @hanoi_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(none) nofree nosync {
  %2 = icmp eq i32 %0, 0
  br i1 %2, label %common.ret, label %3

common.ret:
  %common.op = phi i32 [ %7, %3 ], [ 0, %1 ]
  ret i32 %common.op

3:
  %4 = add nsw i32 %0, -1
  %5 = tail call cc 1002 i32 @hanoi_(i32 inreg noundef %4)
  %6 = shl i32 %5, 1
  %7 = or disjoint i32 %6, 1
  br label %common.ret
}

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = tail call cc 1002 i32 @hanoi_(i32 inreg noundef 13)
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
