; ModuleID = 'tests/run/c/select_of_constants.c'
source_filename = "tests/run/c/select_of_constants.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@__const.main.v = private unnamed_addr constant [5 x i32] [i32 -3, i32 0, i32 1, i32 5, i32 7], align 2

; Function Attrs: noinline nounwind
define dso_local i64 @f302(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = icmp sge i32 %5, %6
  %8 = zext i1 %7 to i64
  %9 = select i1 %7, i64 5, i64 6
  ret i64 %9
}

define dso_local i64 @f334(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = icmp sge i32 %5, %6
  %8 = zext i1 %7 to i64
  %9 = select i1 %7, i64 8, i64 0
  ret i64 %9
}


attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

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
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
