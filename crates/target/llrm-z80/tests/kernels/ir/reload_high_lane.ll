target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 'r2.c'
source_filename = "r2.c"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @f() local_unnamed_addr #0 {
  %1 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr nonnull %1)
  store volatile i16 7, ptr %1, align 2, !tbaa !7
  br label %3

2:                                                ; preds = %15
  call void @llvm.lifetime.end.p0(ptr nonnull %1)
  ret i16 %21

3:                                                ; preds = %0, %15
  %4 = phi i16 [ 0, %0 ], [ %22, %15 ]
  %5 = phi i16 [ 0, %0 ], [ %21, %15 ]
  %6 = tail call i16 @next() #3
  %7 = tail call i16 @next() #3
  %8 = and i16 %7, 3
  switch i16 %8, label %11 [
    i16 0, label %15
    i16 1, label %9
  ]

9:                                                ; preds = %3
  %10 = lshr i16 %6, 8
  br label %15

11:                                               ; preds = %3
  %12 = icmp eq i16 %8, 2
  %13 = select i1 %12, i16 12, i16 13
  %14 = lshr i16 %6, %13
  br label %15

15:                                               ; preds = %3, %9, %11
  %16 = phi i16 [ %14, %11 ], [ %10, %9 ], [ %6, %3 ]
  %17 = udiv i16 %16, 7
  %18 = load volatile i16, ptr %1, align 2, !tbaa !7
  %19 = urem i16 %16, %18
  %20 = add i16 %17, %5
  %21 = add i16 %20, %19
  %22 = add nuw nsw i16 %4, 1
  %23 = icmp eq i16 %22, 100
  br i1 %23, label %2, label %3, !llvm.loop !8
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local i16 @next() local_unnamed_addr #2

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind optsize }

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
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
