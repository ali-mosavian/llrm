target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@seed = internal unnamed_addr global i32 7, align 2
@main.modes = internal unnamed_addr constant [5 x i32] [i32 19, i32 4, i32 6, i32 16, i32 13], align 2

define i32 @gd_read(i32 noundef %0, i32 noundef %1) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" memory(readwrite, argmem: none, inaccessiblemem: none) willreturn nofree nosync norecurse {
  %3 = load i32, ptr @seed, align 2, !tbaa !8
  %4 = mul i32 %3, 1103515245
  %5 = add i32 %4, 12345
  store i32 %5, ptr @seed, align 2, !tbaa !8
  %6 = lshr i32 %5, 16
  %7 = and i32 %6, 15
  %8 = mul i32 %0, 3
  %9 = add i32 %8, %1
  %10 = xor i32 %7, %9
  ret i32 %10
}

define void @pack_ref(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, ptr noundef nocapture nofree %5) nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" memory(readwrite, inaccessiblemem: none) nofree nosync norecurse {
  %7 = icmp eq i32 %0, 16
  %8 = icmp eq i32 %0, 13
  %9 = or i1 %7, %8
  %10 = icmp eq i32 %0, 19
  %11 = icmp eq i32 %0, 4
  %12 = zext i1 %11 to i32
  %13 = select i1 %10, i32 3, i32 %12
  %14 = shl i32 %3, %13
  %15 = add i32 %14, 7
  %16 = lshr i32 %15, 3
  %17 = icmp eq i32 %4, 0
  br i1 %17, label %84, label %18

18:
  %19 = icmp eq i32 %0, 4
  %20 = icmp eq i32 %0, 19
  %21 = select i1 %9, i32 2, i32 0
  %22 = shl nuw nsw i32 %16, %21
  %23 = select i1 %19, i32 6, i32 7
  %24 = select i1 %20, i32 0, i32 %23
  %25 = tail call i32 @llvm.umax.i32(i32 %22, i32 1)
  br label %26

26:
  %27 = phi i32 [ 0, %18 ], [ %81, %80 ]
  %28 = phi ptr [ %5, %18 ], [ %82, %80 ]
  %29 = icmp eq i32 %16, 0
  br i1 %29, label %31, label %30

30:
  tail call void @llvm.memset.p0.i32(ptr align 1 %28, i8 0, i32 %25, i1 false)
  br label %31

31:
  %32 = icmp eq i32 %3, 0
  br i1 %32, label %80, label %33

33:
  %34 = add i32 %27, %2
  br label %35

35:
  %36 = phi i32 [ 0, %33 ], [ %78, %77 ]
  %37 = lshr i32 %36, 3
  %38 = getelementptr i8, ptr %28, i32 %37
  %39 = add i32 %36, %1
  %40 = load i32, ptr @seed, align 2, !tbaa !8
  %41 = mul i32 %40, 1103515245
  %42 = add i32 %41, 12345
  store i32 %42, ptr @seed, align 2, !tbaa !8
  %43 = lshr i32 %42, 16
  %44 = and i32 %43, 15
  %45 = mul i32 %39, 3
  %46 = add i32 %34, %45
  %47 = xor i32 %44, %46
  switch i32 %0, label %66 [
    i32 16, label %48
    i32 13, label %48
  ]

48:
  %49 = and i32 %36, 7
  %50 = lshr exact i32 128, %49
  %51 = trunc nuw i32 %50 to i8
  br label %52

52:
  %53 = phi ptr [ %38, %48 ], [ %65, %61 ]
  %54 = phi i32 [ 0, %48 ], [ %62, %61 ]
  %55 = shl nuw i32 1, %54
  %56 = and i32 %55, %47
  %57 = icmp eq i32 %56, 0
  br i1 %57, label %61, label %58

58:
  %59 = load i8, ptr %53, align 1, !tbaa !12
  %60 = or i8 %59, %51
  store i8 %60, ptr %53, align 1, !tbaa !12
  br label %61

61:
  %62 = add nuw nsw i32 %54, 1
  %63 = icmp samesign ult i32 %54, 3
  %64 = select i1 %9, i1 %63, i1 false
  %65 = getelementptr i8, ptr %53, i32 %16
  br i1 %64, label %52, label %76, !llvm.loop !15

66:
  %67 = shl i32 %36, %13
  %68 = and i32 %67, 7
  %69 = sub nsw i32 %24, %68
  %70 = shl i32 %47, %69
  %71 = lshr i32 %67, 3
  %72 = getelementptr inbounds nuw i8, ptr %28, i32 %71
  %73 = load i8, ptr %72, align 1, !tbaa !12
  %74 = trunc i32 %70 to i8
  %75 = or i8 %73, %74
  store i8 %75, ptr %72, align 1, !tbaa !12
  br label %77

76:
  br label %77

77:
  %78 = add nuw i32 %36, 1
  %exitcond = icmp eq i32 %78, %3
  br i1 %exitcond, label %79, label %35, !llvm.loop !16

79:
  br label %80

80:
  %81 = add nuw i32 %27, 1
  %82 = getelementptr i8, ptr %28, i32 %22
  %exitcond1 = icmp eq i32 %81, %4
  br i1 %exitcond1, label %83, label %26, !llvm.loop !17

83:
  br label %84

84:
  ret void
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" {
  %1 = alloca [400 x i8], align 1
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  br label %2

2:
  %3 = phi i32 [ 0, %0 ], [ %18, %16 ]
  call void @llvm.memset.p0.i32(ptr align 1 %1, i8 -18, i32 400, i1 false)
  %4 = getelementptr inbounds nuw [4 x i8], ptr @main.modes, i32 %3
  %5 = load i32, ptr %4, align 2, !tbaa !8
  call void @pack_ref(i32 noundef %5, i32 noundef 20, i32 noundef 7, i32 noundef 19, i32 noundef 7, ptr noundef %1)
  br label %6

6:
  %7 = phi i32 [ %15, %6 ], [ -400, %2 ]
  %8 = phi ptr [ %14, %6 ], [ %1, %2 ]
  %9 = phi i32 [ 0, %2 ], [ %13, %6 ]
  %10 = mul i32 %9, 31
  %11 = load i8, ptr %8, align 1, !tbaa !12
  %12 = zext i8 %11 to i32
  %13 = add i32 %10, %12
  %14 = getelementptr i8, ptr %8, i32 1
  %15 = add nsw i32 %7, 1
  %exitcond = icmp eq i32 %15, 0
  br i1 %exitcond, label %16, label %6, !llvm.loop !19

16:
  %17 = and i32 %13, 2147483647
  tail call void @report(i32 noundef %17)
  %18 = add nuw nsw i32 %3, 1
  %exitcond1 = icmp eq i32 %18, 5
  br i1 %exitcond1, label %19, label %2, !llvm.loop !20

19:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @report(i32 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881"

declare i32 @llvm.umax.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i32(ptr nocapture writeonly, i8, i32, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

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
!10 = !{!"p1 omnipotent char", !11, i64 0}
!11 = !{!"any pointer", !6, i64 0}
!12 = !{!6, !6, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
!19 = distinct !{!19, !14}
!20 = distinct !{!20, !14}
