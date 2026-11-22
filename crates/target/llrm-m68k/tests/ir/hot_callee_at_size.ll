target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@values = internal constant [4 x i64] [i64 81985529216486895, i64 -1, i64 -9223372036854775807, i64 4294901761], align 8

define i32 @main() nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i64, align 4
  %7 = alloca i64, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  store i32 0, ptr %1, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  store i32 0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i32 0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  store i32 0, ptr %4, align 2, !tbaa !10
  br label %8

8:
  %9 = load i32, ptr %4, align 2, !tbaa !10
  %10 = icmp slt i32 %9, 4
  br i1 %10, label %11, label %46

11:
  store i32 0, ptr %5, align 2, !tbaa !10
  br label %12

12:
  %13 = load i32, ptr %5, align 2, !tbaa !10
  %14 = icmp ult i32 %13, 64
  br i1 %14, label %15, label %42

15:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  %16 = load i32, ptr %4, align 2, !tbaa !10
  %17 = getelementptr inbounds [4 x i64], ptr @values, i32 0, i32 %16
  %18 = load i64, ptr %17, align 8, !tbaa !11
  store i64 %18, ptr %6, align 4, !tbaa !11
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  %19 = load i32, ptr %5, align 2, !tbaa !10
  %20 = zext i32 %19 to i64
  store i64 %20, ptr %7, align 4, !tbaa !11
  %21 = load i32, ptr %1, align 2, !tbaa !8
  %22 = load i64, ptr %6, align 4, !tbaa !11
  %23 = load i64, ptr %7, align 4, !tbaa !11
  %24 = and i64 %23, 31
  %25 = shl i64 %22, %24
  %26 = call i32 @mix(i32 noundef %21, i64 noundef %25) nobuiltin optsize "no-builtins"
  store i32 %26, ptr %1, align 2, !tbaa !8
  %27 = load i32, ptr %2, align 2, !tbaa !8
  %28 = load i64, ptr %6, align 4, !tbaa !11
  %29 = load i64, ptr %7, align 4, !tbaa !11
  %30 = and i64 %29, 31
  %31 = lshr i64 %28, %30
  %32 = call i32 @mix(i32 noundef %27, i64 noundef %31) nobuiltin optsize "no-builtins"
  store i32 %32, ptr %2, align 2, !tbaa !8
  %33 = load i32, ptr %3, align 2, !tbaa !8
  %34 = load i64, ptr %6, align 4, !tbaa !11
  %35 = load i64, ptr %7, align 4, !tbaa !11
  %36 = and i64 %35, 31
  %37 = ashr i64 %34, %36
  %38 = call i32 @mix(i32 noundef %33, i64 noundef %37) nobuiltin optsize "no-builtins"
  store i32 %38, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  br label %39

39:
  %40 = load i32, ptr %5, align 2, !tbaa !10
  %41 = add i32 %40, 1
  store i32 %41, ptr %5, align 2, !tbaa !10
  br label %12, !llvm.loop !13

42:
  br label %43

43:
  %44 = load i32, ptr %4, align 2, !tbaa !10
  %45 = add nsw i32 %44, 1
  store i32 %45, ptr %4, align 2, !tbaa !10
  br label %8, !llvm.loop !15

46:
  %47 = load i32, ptr %1, align 2, !tbaa !8
  call void @report(i32 noundef %47) nobuiltin optsize "no-builtins"
  %48 = load i32, ptr %2, align 2, !tbaa !8
  call void @report(i32 noundef %48) nobuiltin optsize "no-builtins"
  %49 = load i32, ptr %3, align 2, !tbaa !8
  call void @report(i32 noundef %49) nobuiltin optsize "no-builtins"
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal i32 @mix(i32 noundef %0, i64 noundef %1) nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i32, align 2
  %4 = alloca i64, align 4
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i64 %1, ptr %4, align 4, !tbaa !11
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = mul i32 %5, 31
  %7 = load i64, ptr %4, align 4, !tbaa !11
  %8 = trunc i64 %7 to i32
  %9 = add i32 %6, %8
  %10 = load i64, ptr %4, align 4, !tbaa !11
  %11 = lshr i64 %10, 32
  %12 = trunc i64 %11 to i32
  %13 = mul i32 %12, 7
  %14 = add i32 %9, %13
  ret i32 %14
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @report(i32 noundef) optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

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
!10 = !{!5, !5, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"long long", !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
