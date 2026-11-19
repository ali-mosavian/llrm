target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@start = global i32 0, align 2

define i32 @pick(i32 noundef %0, i32 noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %3 = sub i32 %0, 197
  %4 = icmp ule i32 %3, 21
  br i1 %4, label %22, label %5

5:
  %6 = icmp slt i32 %0, 195
  br i1 %6, label %7, label %13

7:
  %8 = icmp eq i32 %0, 192
  br i1 %8, label %24, label %9

9:
  %10 = icmp eq i32 %0, 193
  br i1 %10, label %22, label %11

11:
  %12 = icmp eq i32 %0, 194
  br i1 %12, label %19, label %23

13:
  %14 = icmp eq i32 %0, 195
  br i1 %14, label %22, label %15

15:
  %16 = icmp eq i32 %0, 196
  br i1 %16, label %20, label %17

17:
  %18 = icmp eq i32 %0, 219
  br i1 %18, label %21, label %23

19:
  br label %24

20:
  br label %24

21:
  br label %24

22:
  br label %24

23:
  br label %24

24:
  %25 = phi i32 [ 0, %23 ], [ 4, %19 ], [ 2, %20 ], [ 1, %21 ], [ %1, %22 ], [ 3, %7 ]
  ret i32 %25
}

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load volatile i32, ptr @start, align 2, !tbaa !9
  %2 = icmp slt i32 %1, 256
  br i1 %2, label %3, label %15

3:
  %4 = shl i32 %1, 4
  br label %5

5:
  %6 = phi i32 [ %4, %3 ], [ %14, %12 ]
  %7 = phi i32 [ %13, %12 ], [ %1, %3 ]
  %8 = tail call i32 @pick(i32 noundef %7, i32 noundef 9) nobuiltin "no-builtins"
  %9 = icmp eq i32 %8, 0
  br i1 %9, label %12, label %10

10:
  %11 = add i32 %8, %6
  tail call void @report(i32 noundef %11) nobuiltin "no-builtins"
  br label %12

12:
  %13 = add i32 %7, 1
  %14 = add i32 %6, 16
  %exitcond = icmp eq i32 %13, 256
  br i1 %exitcond, label %15, label %5, !llvm.loop !11

15:
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
