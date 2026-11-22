target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8-S8"

@v = internal global [14 x i32] [i32 0, i32 1, i32 32767, i32 32768, i32 32769, i32 65535, i32 65536, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -1, i32 305419896, i32 -1698898192], align 2

define i16 @main() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" {
  br label %1

1:
  %2 = phi i8 [ %55, %53 ], [ 0, %0 ]
  %3 = phi i8 [ 0, %0 ], [ %54, %53 ]
  %4 = zext i8 %2 to i16
  %5 = getelementptr i8, ptr @v, i16 %4
  %6 = load volatile i32, ptr %5, align 2, !tbaa !8
  %7 = lshr i32 %6, 1
  %8 = mul i32 %6, 514
  %9 = add i32 %8, %7
  %10 = ashr i32 %6, 1
  %11 = mul i32 %9, 257
  %12 = add i32 %11, %10
  %13 = tail call i32 @llvm.fshl.i32(i32 %6, i32 %6, i32 1)
  %14 = mul i32 %12, 257
  %15 = add i32 %14, %13
  %16 = zext i32 %6 to i64
  %17 = shl nuw i64 %16, 32
  %18 = ashr i64 %17, 33
  %19 = trunc nsw i64 %18 to i32
  %20 = lshr i64 %16, 31
  br label %21

21:
  %22 = phi i8 [ %51, %21 ], [ 0, %1 ]
  %23 = phi i32 [ %15, %1 ], [ %50, %21 ]
  %24 = zext i8 %22 to i16
  %25 = getelementptr i8, ptr @v, i16 %24
  %26 = load volatile i32, ptr %25, align 2, !tbaa !8
  %27 = zext i32 %26 to i64
  %28 = or disjoint i64 %17, %27
  %29 = shl i64 %28, 1
  %30 = lshr i64 %28, 1
  %31 = trunc i64 %29 to i32
  %32 = mul i32 %23, 257
  %33 = add i32 %32, %31
  %34 = lshr i64 %29, 32
  %35 = trunc nuw i64 %34 to i32
  %36 = mul i32 %33, 257
  %37 = add i32 %36, %35
  %38 = trunc i64 %30 to i32
  %39 = mul i32 %37, 257
  %40 = add i32 %39, %38
  %41 = mul i32 %40, 257
  %42 = add i32 %41, %7
  %43 = mul i32 %42, 257
  %44 = add i32 %43, %38
  %45 = mul i32 %44, 257
  %46 = add i32 %45, %19
  %47 = or disjoint i64 %29, %20
  %48 = trunc i64 %47 to i32
  %49 = mul i32 %46, 257
  %50 = add i32 %49, %48
  %51 = add nuw nsw i8 %22, 4
  %52 = icmp eq i8 %51, 56
  br i1 %52, label %53, label %21, !llvm.loop !12

53:
  tail call void @report(i32 noundef %50)
  %54 = add nuw nsw i8 %3, 1
  %55 = add nuw nsw i8 %2, 4
  %56 = icmp eq i8 %54, 14
  br i1 %56, label %57, label %1, !llvm.loop !14

57:
  ret i16 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8"

declare i32 @llvm.fshl.i32(i32, i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long long", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
