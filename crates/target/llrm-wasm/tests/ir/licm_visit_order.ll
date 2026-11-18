target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

@seed = internal unnamed_addr global i32 7, align 4

define hidden void @pack_ref(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3, i32 noundef %4, ptr noundef nocapture nofree %5) nounwind memory(readwrite, inaccessiblemem: none) nofree nosync norecurse willreturn {
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
  br i1 %17, label %132, label %18

18:
  %19 = icmp eq i32 %0, 4
  %20 = icmp eq i32 %0, 19
  %promoted1 = load i32, ptr @seed, align 4
  %21 = select i1 %9, i32 2, i32 0
  %22 = shl nuw nsw i32 %16, %21
  %23 = select i1 %19, i32 6, i32 7
  %24 = select i1 %20, i32 0, i32 %23
  %25 = tail call i32 @llvm.umax.i32(i32 %22, i32 1)
  %xtraiter = and i32 %25, 7
  %26 = and i32 %25, 2147483640
  %27 = shl nuw nsw i32 %16, 1
  %28 = mul nuw nsw i32 %16, 3
  %29 = mul i32 %1, 3
  %30 = add i32 %2, %29
  br label %31

31:
  %32 = phi i32 [ %131, %127 ], [ %30, %18 ]
  %33 = phi i32 [ 0, %18 ], [ %129, %127 ]
  %34 = phi ptr [ %5, %18 ], [ %130, %127 ]
  %35 = phi i32 [ %promoted1, %18 ], [ %128, %127 ]
  %36 = icmp eq i32 %16, 0
  br i1 %36, label %47, label %37

37:
  %38 = icmp samesign ult i32 %22, 8
  br i1 %38, label %.epil.preheader, label %39

39:
  br label %50

.unr-lcssa:
  %40 = icmp eq i32 %xtraiter, 0
  br i1 %40, label %47, label %.epil.preheader

.epil.preheader:
  %.epil.init = phi i32 [ 0, %37 ], [ %67, %.unr-lcssa ]
  %41 = icmp ne i32 %xtraiter, 0
  tail call void @llvm.assume(i1 %41)
  %42 = getelementptr i8, ptr %34, i32 %.epil.init
  br label %.epil

.epil:
  %43 = phi i32 [ %46, %.epil ], [ %xtraiter, %.epil.preheader ]
  %44 = phi ptr [ %45, %.epil ], [ %42, %.epil.preheader ]
  store i8 0, ptr %44, align 1, !tbaa !10
  %45 = getelementptr i8, ptr %44, i32 1
  %46 = add nsw i32 %43, -1
  %epil.iter.cmp = icmp eq i32 %46, 0
  br i1 %epil.iter.cmp, label %47, label %.epil

47:
  %48 = icmp eq i32 %3, 0
  br i1 %48, label %127, label %49

49:
  br label %69

50:
  %51 = phi i32 [ %67, %50 ], [ 0, %39 ]
  %52 = getelementptr i8, ptr %34, i32 %51
  store i8 0, ptr %52, align 1, !tbaa !10
  %53 = getelementptr i8, ptr %34, i32 %51
  %54 = getelementptr i8, ptr %53, i32 1
  store i8 0, ptr %54, align 1, !tbaa !10
  %55 = getelementptr i8, ptr %34, i32 %51
  %56 = getelementptr i8, ptr %55, i32 2
  store i8 0, ptr %56, align 1, !tbaa !10
  %57 = getelementptr i8, ptr %34, i32 %51
  %58 = getelementptr i8, ptr %57, i32 3
  store i8 0, ptr %58, align 1, !tbaa !10
  %59 = getelementptr i8, ptr %34, i32 %51
  %60 = getelementptr i8, ptr %59, i32 4
  store i8 0, ptr %60, align 1, !tbaa !10
  %61 = getelementptr i8, ptr %34, i32 %51
  %62 = getelementptr i8, ptr %61, i32 5
  store i8 0, ptr %62, align 1, !tbaa !10
  %63 = getelementptr i8, ptr %34, i32 %51
  %64 = getelementptr i8, ptr %63, i32 6
  store i8 0, ptr %64, align 1, !tbaa !10
  %65 = getelementptr i8, ptr %34, i32 %51
  %66 = getelementptr i8, ptr %65, i32 7
  store i8 0, ptr %66, align 1, !tbaa !10
  %67 = add nuw nsw i32 %51, 8
  %68 = icmp eq i32 %26, %67
  br i1 %68, label %.unr-lcssa, label %50, !llvm.loop !11

69:
  %70 = phi i32 [ %125, %123 ], [ %32, %49 ]
  %71 = phi i32 [ 0, %49 ], [ %124, %123 ]
  %72 = phi i32 [ %35, %49 ], [ %74, %123 ]
  %73 = mul i32 %72, 1103515245
  %74 = add i32 %73, 12345
  %75 = lshr i32 %74, 16
  %76 = and i32 %75, 15
  %77 = xor i32 %76, %70
  switch i32 %0, label %113 [
    i32 16, label %78
    i32 13, label %78
  ]

78:
  %79 = and i32 %71, 7
  %80 = lshr exact i32 128, %79
  %81 = lshr i32 %71, 3
  %invariant.gep = getelementptr inbounds nuw i8, ptr %34, i32 %81
  %82 = and i32 %77, 1
  %83 = icmp eq i32 %82, 0
  br i1 %83, label %88, label %84

84:
  %85 = trunc nuw i32 %80 to i8
  %86 = load i8, ptr %invariant.gep, align 1, !tbaa !10
  %87 = or i8 %86, %85
  store i8 %87, ptr %invariant.gep, align 1, !tbaa !10
  br label %88

88:
  switch i32 %0, label %123 [
    i32 16, label %89
    i32 13, label %89
  ]

89:
  %90 = and i32 %77, 2
  %91 = icmp eq i32 %90, 0
  br i1 %91, label %97, label %92

92:
  %93 = trunc nuw i32 %80 to i8
  %94 = getelementptr inbounds nuw i8, ptr %invariant.gep, i32 %16
  %95 = load i8, ptr %94, align 1, !tbaa !10
  %96 = or i8 %95, %93
  store i8 %96, ptr %94, align 1, !tbaa !10
  br label %97

97:
  %98 = and i32 %77, 4
  %99 = icmp eq i32 %98, 0
  br i1 %99, label %105, label %100

100:
  %101 = trunc nuw i32 %80 to i8
  %102 = getelementptr inbounds nuw i8, ptr %invariant.gep, i32 %27
  %103 = load i8, ptr %102, align 1, !tbaa !10
  %104 = or i8 %103, %101
  store i8 %104, ptr %102, align 1, !tbaa !10
  br label %105

105:
  %106 = and i32 %77, 8
  %107 = icmp eq i32 %106, 0
  br i1 %107, label %123, label %108

108:
  %109 = trunc nuw i32 %80 to i8
  %110 = getelementptr inbounds nuw i8, ptr %invariant.gep, i32 %28
  %111 = load i8, ptr %110, align 1, !tbaa !10
  %112 = or i8 %111, %109
  store i8 %112, ptr %110, align 1, !tbaa !10
  br label %123

113:
  %114 = shl i32 %71, %13
  %115 = and i32 %114, 7
  %116 = sub nsw i32 %24, %115
  %117 = shl i32 %77, %116
  %118 = lshr i32 %114, 3
  %119 = getelementptr inbounds nuw i8, ptr %34, i32 %118
  %120 = load i8, ptr %119, align 1, !tbaa !10
  %121 = trunc i32 %117 to i8
  %122 = or i8 %120, %121
  store i8 %122, ptr %119, align 1, !tbaa !10
  br label %123

123:
  %124 = add nuw i32 %71, 1
  %125 = add i32 %70, 3
  %exitcond1 = icmp eq i32 %3, %124
  br i1 %exitcond1, label %126, label %69, !llvm.loop !14

126:
  store i32 %74, ptr @seed, align 4, !tbaa !6
  br label %127

127:
  %128 = phi i32 [ %74, %126 ], [ %35, %47 ]
  %129 = add nuw i32 %33, 1
  %130 = getelementptr inbounds nuw i8, ptr %34, i32 %22
  %131 = add i32 %32, 1
  %exitcond2 = icmp eq i32 %129, %4
  br i1 %exitcond2, label %132, label %31, !llvm.loop !15

132:
  ret void
}

declare i32 @llvm.umax.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare void @llvm.assume(i1 noundef) nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}
!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!3, !3, i64 0}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 omnipotent char", !9, i64 0}
!9 = !{!"any pointer", !4, i64 0}
!10 = !{!4, !4, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
