target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i64 @sf64_pack(i64 noundef %0, i32 noundef %1, i64 noundef %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) willreturn nofree nosync norecurse {
  %4 = icmp sgt i32 %1, 2046
  br i1 %4, label %5, label %7

5:
  %6 = or i64 %0, 9218868437227405312
  br label %39

7:
  %8 = icmp slt i32 %1, 1
  br i1 %8, label %9, label %24

9:
  %10 = sub nsw i32 1, %1
  %11 = icmp samesign ugt i32 %10, 63
  br i1 %11, label %12, label %15

12:
  %13 = icmp ne i64 %2, 0
  %14 = zext i1 %13 to i64
  br label %24

15:
  %16 = zext nneg i32 %10 to i64
  %17 = lshr i64 %2, %16
  %18 = add nsw i32 %1, 63
  %19 = zext nneg i32 %18 to i64
  %20 = shl i64 %2, %19
  %21 = icmp ne i64 %20, 0
  %22 = zext i1 %21 to i64
  %23 = or i64 %17, %22
  br label %24

24:
  %25 = phi i32 [ %1, %7 ], [ 1, %12 ], [ 1, %15 ]
  %26 = phi i64 [ %2, %7 ], [ %23, %15 ], [ %14, %12 ]
  %27 = and i64 %26, 7
  %28 = lshr i64 %26, 3
  %29 = icmp samesign ugt i64 %27, 4
  %30 = and i64 %26, 15
  %31 = icmp eq i64 %30, 12
  %or.cond1 = or i1 %29, %31
  %32 = zext i1 %or.cond1 to i64
  %33 = add nuw nsw i32 %25, 4095
  %34 = zext nneg i32 %33 to i64
  %35 = shl i64 %34, 52
  %36 = add i64 %28, %0
  %37 = add i64 %36, %35
  %38 = add i64 %37, %32
  br label %39

39:
  %40 = phi i64 [ %6, %5 ], [ %38, %24 ]
  ret i64 %40
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!8 = !{!9, !9, i64 0}
!9 = !{!"long long", !6, i64 0}
!10 = !{!5, !5, i64 0}
