target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

define cc 1002 i32 @bench_rectwo_(i32 inreg noundef %0) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" memory(none) nofree nosync {
  %2 = tail call cc 1002 i32 @paths_(i32 inreg noundef %0, i32 inreg noundef %0)
  ret i32 %2
}

define internal cc 1002 i32 @paths_(i32 inreg noundef %0, i32 inreg noundef %1) nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" nearcode memory(none) nofree nosync {
  %3 = icmp eq i32 %0, 0
  %4 = icmp eq i32 %1, 0
  %5 = or i1 %3, %4
  br i1 %5, label %17, label %6

6:
  %7 = add nsw i32 %0, -1
  br label %8

8:
  %9 = phi i32 [ %1, %6 ], [ %12, %8 ]
  %10 = phi i32 [ 0, %6 ], [ %13, %8 ]
  %11 = tail call cc 1002 i32 @paths_(i32 inreg noundef %7, i32 inreg noundef %9)
  %12 = add nsw i32 %9, -1
  %13 = add i32 %11, %10
  %14 = icmp eq i32 %12, 0
  br i1 %14, label %15, label %8

15:
  %16 = add i32 %13, 1
  br label %17

17:
  %18 = phi i32 [ 1, %2 ], [ %16, %15 ]
  ret i32 %18
}

define cc 1002 i32 @main_() nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" {
  %1 = tail call cc 1002 i32 @paths_(i32 inreg noundef 9, i32 inreg noundef 9)
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
