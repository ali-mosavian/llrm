; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/branch.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/branch.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
define range(i32 -229373, 163827) i32 @parity_branch(ptr nocapture noundef readonly %value) local_unnamed_addr #0 {
entry:
  %0 = load i16, ptr %value, align 2, !tbaa !2
  %cmp = icmp slt i16 %0, 0
  br i1 %cmp, label %if.then, label %if.end

if.then:                                          ; preds = %entry
  %conv = sext i16 %0 to i32
  %mul = mul nsw i32 %conv, 7
  %add = add nuw nsw i32 %mul, 3
  br label %return

if.end:                                           ; preds = %entry
  %conv1 = zext nneg i16 %0 to i32
  %mul2 = mul nuw nsw i32 %conv1, 5
  %sub = add nsw i32 %mul2, -9
  br label %return

return:                                           ; preds = %if.end, %if.then
  %retval.0 = phi i32 [ %add, %if.then ], [ %sub, %if.end ]
  ret i32 %retval.0
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(write, argmem: none, inaccessiblemem: none)
define noundef range(i32 -229602373, 163989827) i32 @parity_branch_demo() local_unnamed_addr #1 {
entry:
  ret i32 -87904
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
