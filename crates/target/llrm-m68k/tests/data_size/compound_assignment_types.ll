target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@uc = global i8 50, align 1
@sy = global i16 -5, align 2
@uy = global i8 7, align 1
@ly = global i32 -3, align 2
@us = global i16 -25536, align 2
@sc = global i8 -100, align 1
@shift = global i32 3, align 2

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load volatile i16, ptr @sy, align 2, !tbaa !10
  %2 = load i8, ptr @uc, align 1, !tbaa !12
  %3 = zext i8 %2 to i16
  %4 = sdiv i16 %3, %1
  %5 = and i16 %4, 255
  %6 = load volatile i16, ptr @sy, align 2, !tbaa !10
  %7 = srem i16 200, %6
  %8 = mul nuw nsw i16 %5, 7
  %9 = add nuw nsw i16 %7, %8
  %10 = load volatile i32, ptr @ly, align 2, !tbaa !8
  %11 = load i16, ptr @us, align 2, !tbaa !10
  %12 = zext i16 %11 to i32
  %13 = sdiv i32 %12, %10
  %14 = trunc i32 %13 to i16
  store i16 %14, ptr @us, align 2, !tbaa !10
  %15 = mul nuw nsw i16 %9, 7
  %16 = zext nneg i16 %15 to i32
  %17 = and i32 %13, 65535
  %18 = add nuw nsw i32 %17, %16
  %19 = load volatile i8, ptr @uy, align 1, !tbaa !12
  %20 = load i8, ptr @sc, align 1, !tbaa !12
  %21 = sext i8 %20 to i16
  %22 = zext i8 %19 to i16
  %23 = sdiv i16 %21, %22
  %.sext2 = sext i16 %23 to i32
  %24 = mul nuw nsw i32 %18, 7
  %25 = add nsw i32 %24, %.sext2
  %26 = load volatile i32, ptr @shift, align 2, !tbaa !13
  %27 = ashr i32 -100, %26
  %28 = trunc nsw i32 %27 to i8
  store i8 %28, ptr @sc, align 1, !tbaa !12
  %29 = mul nsw i32 %25, 7
  %30 = add nsw i32 %29, %27
  %31 = load volatile i32, ptr @shift, align 2, !tbaa !13
  %32 = shl i32 129, %31
  %33 = trunc i32 %32 to i8
  store i8 %33, ptr @uc, align 1, !tbaa !12
  %34 = mul nsw i32 %30, 7
  %35 = and i32 %32, 255
  %36 = add nsw i32 %35, %34
  tail call void @report(i32 noundef %36) nobuiltin "no-builtins"
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
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"short", !6, i64 0}
!12 = !{!6, !6, i64 0}
!13 = !{!5, !5, i64 0}
