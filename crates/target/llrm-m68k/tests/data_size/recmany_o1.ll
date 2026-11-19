target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @bench_recmany(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync willreturn norecurse {
  %2 = icmp eq i32 %0, 0
  br i1 %2, label %26, label %3

3:
  br label %4

4:
  %5 = phi i32 [ %19, %4 ], [ %0, %3 ]
  %6 = phi i32 [ %18, %4 ], [ 5, %3 ]
  %7 = phi i32 [ %16, %4 ], [ 4, %3 ]
  %8 = phi i32 [ %14, %4 ], [ 3, %3 ]
  %9 = phi i32 [ %13, %4 ], [ 2, %3 ]
  %10 = phi i32 [ %11, %4 ], [ 1, %3 ]
  %11 = add nsw i32 %9, 1
  %12 = srem i32 %10, 5
  %13 = add nsw i32 %12, %8
  %14 = xor i32 %9, %7
  %15 = srem i32 %8, 3
  %16 = add nsw i32 %15, %6
  %17 = srem i32 %7, 7
  %18 = add nsw i32 %10, %17
  %19 = add nsw i32 %5, -1
  %20 = icmp eq i32 %19, 0
  br i1 %20, label %21, label %4

21:
  %22 = add nsw i32 %13, %11
  %23 = add nsw i32 %22, %14
  %24 = add nsw i32 %23, %16
  %25 = add nsw i32 %24, %18
  br label %26

26:
  %27 = phi i32 [ %25, %21 ], [ 15, %1 ]
  ret i32 %27
}

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %1

1:
  %2 = phi i32 [ -1500, %0 ], [ %16, %1 ]
  %3 = phi i32 [ %15, %1 ], [ 5, %0 ]
  %4 = phi i32 [ %13, %1 ], [ 4, %0 ]
  %5 = phi i32 [ %11, %1 ], [ 3, %0 ]
  %6 = phi i32 [ %10, %1 ], [ 2, %0 ]
  %7 = phi i32 [ %8, %1 ], [ 1, %0 ]
  %8 = add nsw i32 %6, 1
  %9 = srem i32 %7, 5
  %10 = add nsw i32 %9, %5
  %11 = xor i32 %6, %4
  %12 = srem i32 %5, 3
  %13 = add nsw i32 %12, %3
  %14 = srem i32 %4, 7
  %15 = add nsw i32 %7, %14
  %16 = add nsw i32 %2, 1
  %17 = icmp eq i32 %16, 0
  br i1 %17, label %18, label %1

18:
  %19 = add i32 %11, %8
  %20 = add i32 %19, %13
  %21 = add i32 %20, %15
  %22 = add i32 %21, %10
  tail call void @report(i32 noundef %22) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !6, i64 0}
