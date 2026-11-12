target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@s = internal unnamed_addr global i64 88172645463325252, align 8

define void @mx(i32 noundef %0, i32 noundef %1, ptr noundef nocapture nofree writeonly initializes((0, 4)) "llrm.reads"="" "llrm.writes"="0:4" %2, ptr noundef nocapture nofree writeonly initializes((0, 4)) "llrm.reads"="" "llrm.writes"="0:4" %3) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" willreturn nofree nosync norecurse memory(argmem: write) {
  %5 = shl i32 %1, 3
  %6 = xor i32 %5, %0
  %7 = lshr i32 %1, 5
  %8 = xor i32 %6, %7
  store i32 %8, ptr %2, align 2, !tbaa !8
  %9 = lshr i32 %0, 7
  %10 = mul i32 %1, 3
  %11 = add i32 %10, %9
  store i32 %11, ptr %3, align 2, !tbaa !8
  ret void
}

define i64 @f(i64 noundef %0, i64 noundef %1) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" willreturn nofree nosync norecurse memory(none) {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %4) nounwind
  %5 = trunc i64 %0 to i32
  %6 = trunc i64 %1 to i32
  call void @mx(i32 noundef %5, i32 noundef %6, ptr noundef nonnull %3, ptr noundef nonnull %4) nobuiltin "no-builtins"
  %7 = load i32, ptr %3, align 2, !tbaa !8
  %8 = zext i32 %7 to i64
  %9 = lshr i64 %1, 32
  %10 = trunc nuw i64 %9 to i32
  call void @mx(i32 noundef %5, i32 noundef %10, ptr noundef nonnull %3, ptr noundef nonnull %4) nobuiltin "no-builtins"
  %11 = load i32, ptr %3, align 2, !tbaa !8
  %12 = zext i32 %11 to i64
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = zext i32 %13 to i64
  %15 = lshr i64 %0, 32
  %16 = trunc nuw i64 %15 to i32
  call void @mx(i32 noundef %16, i32 noundef %6, ptr noundef nonnull %3, ptr noundef nonnull %4) nobuiltin "no-builtins"
  %17 = load i32, ptr %3, align 2, !tbaa !8
  %18 = zext i32 %17 to i64
  %19 = load i32, ptr %4, align 2, !tbaa !8
  %20 = zext i32 %19 to i64
  call void @mx(i32 noundef %16, i32 noundef %10, ptr noundef nonnull %3, ptr noundef nonnull %4) nobuiltin "no-builtins"
  %21 = load i32, ptr %3, align 2, !tbaa !8
  %22 = zext i32 %21 to i64
  %23 = shl nuw i64 %22, 32
  %24 = load i32, ptr %4, align 2, !tbaa !8
  %25 = zext i32 %24 to i64
  %26 = or disjoint i64 %23, %25
  %27 = add nuw nsw i64 %14, %8
  %28 = add nuw nsw i64 %27, %20
  %29 = lshr i64 %28, 32
  %30 = add nuw nsw i64 %18, %12
  %31 = add nuw nsw i64 %30, %29
  %32 = add i64 %31, %26
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %3) nounwind
  ret i64 %32
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  br label %2

1:
  ret i32 0

2:
  %3 = phi i32 [ %21, %2 ], [ 200, %0 ]
  %4 = load i64, ptr @s, align 8, !tbaa !12
  %5 = shl i64 %4, 13
  %6 = xor i64 %5, %4
  %7 = lshr i64 %6, 7
  %8 = xor i64 %7, %6
  %9 = shl i64 %8, 17
  %10 = xor i64 %9, %8
  %11 = shl i64 %10, 13
  %12 = xor i64 %11, %10
  %13 = lshr i64 %12, 7
  %14 = xor i64 %13, %12
  %15 = shl i64 %14, 17
  %16 = xor i64 %15, %14
  store i64 %16, ptr @s, align 8, !tbaa !12
  %17 = tail call i64 @f(i64 noundef %10, i64 noundef %16) nobuiltin "no-builtins"
  %18 = lshr i64 %17, 32
  %19 = trunc nuw i64 %18 to i32
  tail call void @report(i32 noundef %19) nobuiltin "no-builtins"
  %20 = trunc i64 %17 to i32
  tail call void @report(i32 noundef %20) nobuiltin "no-builtins"
  %21 = add nsw i32 %3, -1
  %exitcond = icmp eq i32 %21, 0
  br i1 %exitcond, label %1, label %2, !llvm.loop !14
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
!10 = !{!"p1 int", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"long long", !6, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
