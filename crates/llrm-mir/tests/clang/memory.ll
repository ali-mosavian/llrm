; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/memory.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/memory.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
define range(i32 -1073709056, 1073741825) i32 @parity_memory(ptr nocapture noundef %value, ptr nocapture noundef readonly %delta) local_unnamed_addr #0 {
entry:
  %0 = load i16, ptr %value, align 2, !tbaa !2
  %mul = mul nsw i16 %0, 3
  %1 = load i16, ptr %delta, align 2, !tbaa !2
  %add = add nsw i16 %mul, %1
  store i16 %add, ptr %value, align 2, !tbaa !2
  %conv = sext i16 %add to i32
  %mul2 = mul nsw i32 %conv, %conv
  ret i32 %mul2
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none)
define noundef i32 @parity_memory_demo() local_unnamed_addr #1 {
entry:
  ret i32 361001
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"short", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
