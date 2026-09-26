; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/qmove.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/qmove.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
define void @pl_ground_accel(ptr nocapture noundef %vel, ptr nocapture noundef readonly %wishdir, float noundef %wishspeed, float noundef %dt) local_unnamed_addr #0 {
entry:
  %0 = load float, ptr %vel, align 2, !tbaa !2
  %1 = load float, ptr %wishdir, align 2, !tbaa !2
  %y = getelementptr inbounds nuw i8, ptr %vel, i16 4
  %2 = load float, ptr %y, align 2, !tbaa !7
  %y2 = getelementptr inbounds nuw i8, ptr %wishdir, i16 4
  %3 = load float, ptr %y2, align 2, !tbaa !7
  %mul3 = fmul float %2, %3
  %4 = tail call float @llvm.fmuladd.f32(float %0, float %1, float %mul3)
  %sub = fsub float %wishspeed, %4
  %cmp = fcmp ugt float %sub, 0.000000e+00
  br i1 %cmp, label %if.end, label %cleanup

if.end:                                           ; preds = %entry
  %mul = fmul float %wishspeed, 1.000000e+01
  %mul4 = fmul float %mul, %dt
  %cmp5 = fcmp ogt float %mul4, %sub
  %accelspeed.0 = select i1 %cmp5, float %sub, float %mul4
  %5 = tail call float @llvm.fmuladd.f32(float %accelspeed.0, float %1, float %0)
  store float %5, ptr %vel, align 2, !tbaa !2
  %6 = tail call float @llvm.fmuladd.f32(float %accelspeed.0, float %3, float %2)
  store float %6, ptr %y, align 2, !tbaa !7
  br label %cleanup

cleanup:                                          ; preds = %entry, %if.end
  ret void
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #1

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define noundef i32 @quake_move_demo() local_unnamed_addr #2 {
entry:
  ret i32 100405
}

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"", !4, i64 0, !4, i64 4, !4, i64 8}
!4 = !{!"float", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!3, !4, i64 4}
