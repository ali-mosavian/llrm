; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/algebra.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/algebra.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
define range(i32 -1409023, 1408983) i32 @parity_algebra(ptr nocapture noundef readonly %a, ptr nocapture noundef readonly %b) local_unnamed_addr #0 {
entry:
  %0 = load i16, ptr %a, align 2, !tbaa !2
  %conv = sext i16 %0 to i32
  %mul = mul nsw i32 %conv, 9
  %1 = load i16, ptr %b, align 2, !tbaa !2
  %conv1 = sext i16 %1 to i32
  %mul2 = mul nsw i32 %conv1, 5
  %add = add nsw i32 %mul2, %mul
  %mul3 = mul nsw i32 %add, 3
  %sub = sub nsw i32 %mul3, %conv
  ret i32 %sub
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none)
define noundef range(i32 -1410432023, 1410390983) i32 @parity_algebra_demo() local_unnamed_addr #1 {
entry:
  ret i32 702774
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
