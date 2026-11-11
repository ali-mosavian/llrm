target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@b = global [160 x i16] zeroinitializer, align 16
@a = global [160 x i16] zeroinitializer, align 16

define void @copy(i16 noundef signext %0) nounwind {
  %2 = icmp sgt i16 %0, 0
  br i1 %2, label %3, label %32

3:
  %wide.trip.count = zext nneg i16 %0 to i32
  %xtraiter = and i32 %wide.trip.count, 3
  %4 = icmp ult i16 %0, 4
  br i1 %4, label %.epil.preheader, label %.new

.new:
  %5 = and i32 %wide.trip.count, 32764
  br label %6

6:
  %7 = phi i32 [ 0, %.new ], [ %23, %6 ]
  %niter = phi i32 [ 0, %.new ], [ %24, %6 ]
  %8 = getelementptr inbounds nuw [2 x i8], ptr @b, i32 %7
  %9 = load i16, ptr %8, align 8, !tbaa !6
  %10 = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %7
  store i16 %9, ptr %10, align 8, !tbaa !6
  %11 = or disjoint i32 %7, 1
  %12 = getelementptr inbounds nuw [2 x i8], ptr @b, i32 %11
  %13 = load i16, ptr %12, align 2, !tbaa !6
  %14 = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %11
  store i16 %13, ptr %14, align 2, !tbaa !6
  %15 = or disjoint i32 %7, 2
  %16 = getelementptr inbounds nuw [2 x i8], ptr @b, i32 %15
  %17 = load i16, ptr %16, align 4, !tbaa !6
  %18 = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %15
  store i16 %17, ptr %18, align 4, !tbaa !6
  %19 = or disjoint i32 %7, 3
  %20 = getelementptr inbounds nuw [2 x i8], ptr @b, i32 %19
  %21 = load i16, ptr %20, align 2, !tbaa !6
  %22 = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %19
  store i16 %21, ptr %22, align 2, !tbaa !6
  %23 = add nuw nsw i32 %7, 4
  %24 = add i32 %niter, 4
  %25 = icmp eq i32 %24, %5
  br i1 %25, label %.unr-lcssa, label %6, !llvm.loop !8

.unr-lcssa:
  %lcmp.mod = icmp eq i32 %xtraiter, 0
  br i1 %lcmp.mod, label %32, label %.epil.preheader

.epil.preheader:
  %.epil.init = phi i32 [ 0, %3 ], [ %23, %.unr-lcssa ]
  %lcmp.mod1 = icmp ne i32 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod1)
  br label %.epil

.epil:
  %26 = phi i32 [ %.epil.init, %.epil.preheader ], [ %30, %.epil ]
  %epil.iter = phi i32 [ 0, %.epil.preheader ], [ %epil.iter.next, %.epil ]
  %27 = getelementptr inbounds nuw [2 x i8], ptr @b, i32 %26
  %28 = load i16, ptr %27, align 2, !tbaa !6
  %29 = getelementptr inbounds nuw [2 x i8], ptr @a, i32 %26
  store i16 %28, ptr %29, align 2, !tbaa !6
  %30 = add nuw nsw i32 %26, 1
  %epil.iter.next = add i32 %epil.iter, 1
  %epil.iter.cmp = icmp eq i32 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp, label %31, label %.epil

31:
  br label %32

32:
  ret void
}



declare void @llvm.assume(i1 noundef) nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
