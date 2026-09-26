; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/parity.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/parity.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

%struct.Pair = type { i16, i16 }

; Function Attrs: nofree norecurse nosync nounwind memory(none)
define i32 @parity_kernel() local_unnamed_addr #0 {
entry:
  %points = alloca [8 x %struct.Pair], align 2
  call void @llvm.lifetime.start.p0(i64 32, ptr nonnull %points) #2
  br label %for.body

for.body:                                         ; preds = %entry, %for.body
  %index.026 = phi i16 [ 0, %entry ], [ %inc, %for.body ]
  %mul = mul nuw nsw i16 %index.026, 3
  %add = add nuw nsw i16 %mul, 1
  %arrayidx = getelementptr inbounds nuw [8 x %struct.Pair], ptr %points, i16 0, i16 %index.026
  store i16 %add, ptr %arrayidx, align 2, !tbaa !2
  %mul1 = shl nuw nsw i16 %index.026, 1
  %sub = sub nuw nsw i16 29, %mul1
  %y = getelementptr inbounds nuw i8, ptr %arrayidx, i16 2
  store i16 %sub, ptr %y, align 2, !tbaa !7
  %inc = add nuw nsw i16 %index.026, 1
  %exitcond.not = icmp eq i16 %inc, 8
  br i1 %exitcond.not, label %for.body5, label %for.body, !llvm.loop !8

for.body5:                                        ; preds = %for.body, %for.body5
  %index.128 = phi i16 [ %inc14, %for.body5 ], [ 0, %for.body ]
  %total.027 = phi i32 [ %add12, %for.body5 ], [ 17, %for.body ]
  %arrayidx6 = getelementptr inbounds nuw [8 x %struct.Pair], ptr %points, i16 0, i16 %index.128
  %0 = load i16, ptr %arrayidx6, align 2, !tbaa !2
  %conv = sext i16 %0 to i32
  %y9 = getelementptr inbounds nuw i8, ptr %arrayidx6, i16 2
  %1 = load i16, ptr %y9, align 2, !tbaa !7
  %conv10 = sext i16 %1 to i32
  %mul11 = mul nsw i32 %conv10, %conv
  %add12 = add nsw i32 %mul11, %total.027
  %inc14 = add nuw nsw i16 %index.128, 1
  %exitcond29.not = icmp eq i16 %inc14, 8
  br i1 %exitcond29.not, label %for.end15, label %for.body5, !llvm.loop !11

for.end15:                                        ; preds = %for.body5
  call void @llvm.lifetime.end.p0(i64 32, ptr nonnull %points) #2
  ret i32 %add12
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nofree norecurse nosync nounwind memory(none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"", !4, i64 0, !4, i64 2}
!4 = !{!"short", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!3, !4, i64 2}
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
!11 = distinct !{!11, !9, !10}
