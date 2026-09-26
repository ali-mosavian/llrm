; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/loop.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/loop.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
define i32 @parity_loop(ptr nocapture noundef readonly %count, ptr nocapture noundef readonly %seed) local_unnamed_addr #0 {
entry:
  %0 = load i16, ptr %seed, align 2, !tbaa !2
  %conv = sext i16 %0 to i32
  %1 = load i16, ptr %count, align 2, !tbaa !2
  %cmp9 = icmp sgt i16 %1, 0
  br i1 %cmp9, label %while.end.loopexit, label %while.end

while.end.loopexit:                               ; preds = %entry
  %2 = add nsw i16 %1, -1
  %3 = zext i16 %2 to i32
  %4 = add nsw i16 %1, -2
  %5 = zext i16 %4 to i32
  %6 = mul nuw i32 %3, %5
  %7 = lshr i32 %6, 1
  %8 = add nuw nsw i32 %7, 1
  %9 = mul i32 %8, %conv
  %10 = add nsw i32 %conv, 3
  %11 = zext i16 %2 to i32
  %12 = mul i32 %10, %11
  %13 = add i32 %9, %12
  %14 = add i32 %13, 3
  br label %while.end

while.end:                                        ; preds = %while.end.loopexit, %entry
  %total.0.lcssa = phi i32 [ %conv, %entry ], [ %14, %while.end.loopexit ]
  ret i32 %total.0.lcssa
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none)
define noundef i32 @parity_loop_demo() local_unnamed_addr #1 {
entry:
  ret i32 130991
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"short", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
