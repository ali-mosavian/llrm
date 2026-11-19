; target: triple=i486-unknown-linux-gnu cpu=i486 features=-sse,-sse2,-mmx datalayout=e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32 optsize=false
; ModuleID = '<stdin>'
source_filename = "/tmp/claude-1000/rp/one.c"
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"
target triple = "i486-unknown-linux-gnu"

@sum = internal unnamed_addr global i32 0, align 4
@state = internal unnamed_addr global i32 -1831433054, align 4

; Function Attrs: nounwind
define dso_local noundef i32 @main() local_unnamed_addr #0 {
  %1 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr nonnull %1)
  store volatile i32 3, ptr %1, align 4, !tbaa !9
  %state.promoted.i = load i32, ptr @state, align 4, !tbaa !9
  %sum.promoted.i = load i32, ptr @sum, align 4
  br label %2

2:                                                ; preds = %35, %0
  %.056.i = phi i32 [ 0, %0 ], [ %39, %35 ]
  %.04955.i = phi i32 [ 0, %0 ], [ %.150.i, %35 ]
  %3 = phi i32 [ %state.promoted.i, %0 ], [ %16, %35 ]
  %4 = phi i32 [ %sum.promoted.i, %0 ], [ %38, %35 ]
  %5 = shl i32 %3, 13
  %6 = xor i32 %5, %3
  %7 = lshr i32 %6, 17
  %8 = xor i32 %7, %6
  %9 = shl i32 %8, 5
  %10 = xor i32 %9, %8
  %11 = shl i32 %10, 13
  %12 = xor i32 %11, %10
  %13 = lshr i32 %12, 17
  %14 = xor i32 %13, %12
  %15 = shl i32 %14, 5
  %16 = xor i32 %15, %14
  store i32 %16, ptr @state, align 4, !tbaa !9
  %17 = and i32 %14, 7
  switch i32 %17, label %22 [
    i32 0, label %26
    i32 1, label %18
    i32 2, label %20
  ]

18:                                               ; preds = %2
  %19 = lshr i32 %10, 16
  br label %26

20:                                               ; preds = %2
  %21 = lshr i32 %10, 24
  br label %26

22:                                               ; preds = %2
  %23 = icmp eq i32 %17, 3
  %24 = lshr i32 %10, 29
  %25 = select i1 %23, i32 %24, i32 %10
  br label %26

26:                                               ; preds = %22, %20, %18, %2
  %27 = phi i32 [ %25, %22 ], [ %19, %18 ], [ %21, %20 ], [ %10, %2 ]
  %28 = udiv i32 %27, 3
  %29 = urem i32 %27, 3
  %.0..0..0..0..0..0..i = load volatile i32, ptr %1, align 4, !tbaa !9
  %30 = udiv i32 %27, %.0..0..0..0..0..0..i
  %.not53.i = icmp eq i32 %28, %30
  br i1 %.not53.i, label %31, label %33

31:                                               ; preds = %26
  %.0..0..0..0..0..0.46.i = load volatile i32, ptr %1, align 4, !tbaa !9
  %32 = urem i32 %27, %.0..0..0..0..0..0.46.i
  %.not54.i = icmp eq i32 %29, %32
  br i1 %.not54.i, label %35, label %33

33:                                               ; preds = %31, %26
  %34 = add nsw i32 %.04955.i, 1
  br label %35

35:                                               ; preds = %33, %31
  %.150.i = phi i32 [ %34, %33 ], [ %.04955.i, %31 ]
  %36 = mul i32 %28, 31
  %37 = add i32 %29, %4
  %38 = add i32 %37, %36
  store i32 %38, ptr @sum, align 4, !tbaa !9
  %39 = add nuw nsw i32 %.056.i, 1
  %exitcond.not.i = icmp eq i32 %39, 1500
  br i1 %exitcond.not.i, label %.preheader.i, label %2, !llvm.loop !11

.preheader.i:                                     ; preds = %35, %69
  %.159.i = phi i32 [ %73, %69 ], [ 0, %35 ]
  %.258.i = phi i32 [ %.3.i, %69 ], [ %.150.i, %35 ]
  %40 = phi i32 [ %72, %69 ], [ %38, %35 ]
  %41 = icmp samesign ult i32 %.159.i, 2
  br i1 %41, label %60, label %42

42:                                               ; preds = %.preheader.i
  %43 = icmp samesign ult i32 %.159.i, 4
  br i1 %43, label %44, label %46

44:                                               ; preds = %42
  %45 = sub nsw i32 1, %.159.i
  br label %60

46:                                               ; preds = %42
  %47 = icmp samesign ult i32 %.159.i, 6
  br i1 %47, label %48, label %50

48:                                               ; preds = %46
  %49 = add nsw i32 %.159.i, -2
  br label %60

50:                                               ; preds = %46
  %51 = icmp samesign ult i32 %.159.i, 8
  br i1 %51, label %52, label %54

52:                                               ; preds = %50
  %53 = sub nsw i32 5, %.159.i
  br label %60

54:                                               ; preds = %50
  %55 = icmp samesign ult i32 %.159.i, 10
  br i1 %55, label %56, label %58

56:                                               ; preds = %54
  %57 = add nuw nsw i32 %.159.i, -12
  br label %60

58:                                               ; preds = %54
  %59 = sub nuw nsw i32 16, %.159.i
  br label %60

60:                                               ; preds = %58, %56, %52, %48, %44, %.preheader.i
  %61 = phi i32 [ %59, %58 ], [ %45, %44 ], [ %49, %48 ], [ %53, %52 ], [ %57, %56 ], [ %.159.i, %.preheader.i ]
  %62 = udiv i32 %61, 3
  %63 = urem i32 %61, 3
  %.0..0..0..0..0..0.47.i = load volatile i32, ptr %1, align 4, !tbaa !9
  %64 = udiv i32 %61, %.0..0..0..0..0..0.47.i
  %.not.i = icmp eq i32 %62, %64
  br i1 %.not.i, label %65, label %67

65:                                               ; preds = %60
  %.0..0..0..0..0..0.48.i = load volatile i32, ptr %1, align 4, !tbaa !9
  %66 = urem i32 %61, %.0..0..0..0..0..0.48.i
  %.not52.i = icmp eq i32 %63, %66
  br i1 %.not52.i, label %69, label %67

67:                                               ; preds = %65, %60
  %68 = add nsw i32 %.258.i, 1
  br label %69

69:                                               ; preds = %67, %65
  %.3.i = phi i32 [ %68, %67 ], [ %.258.i, %65 ]
  %70 = mul i32 %62, 31
  %71 = add i32 %63, %40
  %72 = add i32 %71, %70
  store i32 %72, ptr @sum, align 4, !tbaa !9
  %73 = add nuw nsw i32 %.159.i, 1
  %exitcond60.not.i = icmp eq i32 %73, 12
  br i1 %exitcond60.not.i, label %c_ulong_3.exit, label %.preheader.i, !llvm.loop !13

c_ulong_3.exit:                                   ; preds = %69
  call void @llvm.lifetime.end.p0(ptr nonnull %1)
  tail call void @report(i32 noundef %.3.i) #3
  %74 = load i32, ptr @sum, align 4, !tbaa !9
  tail call void @report(i32 noundef %74) #3
  ret i32 0
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) local_unnamed_addr #2

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind "min-legal-vector-width"="0" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="-sse,-sse2,-mmx" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nobuiltin nounwind "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 1, !"wchar_size", i32 2}
!2 = !{i32 1, !"override-stack-alignment", i32 4}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !7, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = distinct !{!13, !12}
