target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @bench_hanoi(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync {
  %2 = tail call fastcc i32 @hanoi(i32 noundef %0) nobuiltin "no-builtins"
  ret i32 %2
}

define internal fastcc i32 @hanoi(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync {
  %2 = icmp eq i32 %0, 0
  br i1 %2, label %common.ret, label %3

common.ret:
  %common.op = phi i32 [ %7, %3 ], [ 0, %1 ]
  ret i32 %common.op

3:
  %4 = add nsw i32 %0, -1
  %5 = tail call fastcc i32 @hanoi(i32 noundef %4) nobuiltin "no-builtins"
  %6 = shl i32 %5, 1
  %7 = or disjoint i32 %6, 1
  br label %common.ret
}

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
  %1 = tail call fastcc i32 @hanoi(i32 noundef 13) nobuiltin "no-builtins"
  tail call void @report(i32 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext"

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}

