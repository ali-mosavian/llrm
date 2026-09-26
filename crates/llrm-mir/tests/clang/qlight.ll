; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/qlight.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/qlight.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define signext range(i16 0, 256) i16 @ls_scale_byte(i16 noundef signext %raw, i16 noundef signext %sval) local_unnamed_addr #0 {
entry:
  %conv = sext i16 %raw to i32
  %conv1 = sext i16 %sval to i32
  %mul = mul nsw i32 %conv1, %conv
  %div = sdiv i32 %mul, 120
  %spec.store.select = tail call i32 @llvm.smin.i32(i32 %div, i32 255)
  %spec.store.select8 = tail call i32 @llvm.smax.i32(i32 %spec.store.select, i32 0)
  %conv7 = trunc nuw nsw i32 %spec.store.select8 to i16
  ret i16 %conv7
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define noundef i32 @quake_light_demo() local_unnamed_addr #0 {
entry:
  ret i32 200100255
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smin.i32(i32, i32) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smax.i32(i32, i32) #1

attributes #0 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
