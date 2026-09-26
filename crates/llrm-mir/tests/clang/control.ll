; ModuleID = '/home/alim/work/personal/llrm-zed/bench/parity/control.c'
source_filename = "/home/alim/work/personal/llrm-zed/bench/parity/control.c"
target datalayout = "e-m:e-p:16:16-i32:16-i64:16-f32:16-f64:16-a:8-n8:16-S16"

; Function Attrs: nofree norecurse nosync nounwind memory(argmem: read)
define i32 @parity_control(ptr nocapture noundef readonly %value, ptr nocapture noundef readonly %limit) local_unnamed_addr #0 {
entry:
  %0 = load i16, ptr %limit, align 2, !tbaa !2
  %cmp14 = icmp sgt i16 %0, 0
  br i1 %cmp14, label %for.body.preheader, label %for.end

for.body.preheader:                               ; preds = %entry
  %1 = load i16, ptr %value, align 2, !tbaa !2
  %conv = sext i16 %1 to i32
  %2 = sub nsw i32 0, %conv
  br label %for.body

for.body:                                         ; preds = %for.body.preheader, %for.body
  %index.016 = phi i16 [ %inc, %for.body ], [ 0, %for.body.preheader ]
  %total.015 = phi i32 [ %total.1, %for.body ], [ 0, %for.body.preheader ]
  %and = and i16 %index.016, 1
  %cmp1 = icmp eq i16 %and, 0
  %conv2 = zext nneg i16 %index.016 to i32
  %add = add i32 %total.015, %conv2
  %total.1.p = select i1 %cmp1, i32 %conv, i32 %2
  %total.1 = add i32 %add, %total.1.p
  %inc = add nuw nsw i16 %index.016, 1
  %exitcond.not = icmp eq i16 %inc, %0
  br i1 %exitcond.not, label %for.end, label %for.body, !llvm.loop !6

for.end:                                          ; preds = %for.body, %entry
  %total.0.lcssa = phi i32 [ 0, %entry ], [ %total.1, %for.body ]
  ret i32 %total.0.lcssa
}

; Function Attrs: nofree norecurse nosync nounwind memory(write, argmem: none, inaccessiblemem: none)
define i32 @parity_control_demo() local_unnamed_addr #1 {
entry:
  br label %for.body.i

for.body.i:                                       ; preds = %entry, %for.inc.i
  %index.016.i = phi i16 [ %inc.i, %for.inc.i ], [ 0, %entry ]
  %total.015.i = phi i32 [ %total.1.i, %for.inc.i ], [ 0, %entry ]
  %and.i = and i16 %index.016.i, 1
  %cmp1.i = icmp eq i16 %and.i, 0
  br i1 %cmp1.i, label %if.then.i, label %if.else.i

if.then.i:                                        ; preds = %for.body.i
  %narrow = add nuw nsw i16 %index.016.i, 7
  %add.i = zext nneg i16 %narrow to i32
  %add3.i = add i32 %total.015.i, %add.i
  br label %for.inc.i

if.else.i:                                        ; preds = %for.body.i
  %conv5.i = zext nneg i16 %index.016.i to i32
  %sub.neg.i = add nsw i32 %conv5.i, -7
  %sub6.i = add i32 %sub.neg.i, %total.015.i
  br label %for.inc.i

for.inc.i:                                        ; preds = %if.else.i, %if.then.i
  %total.1.i = phi i32 [ %add3.i, %if.then.i ], [ %sub6.i, %if.else.i ]
  %inc.i = add nuw nsw i16 %index.016.i, 1
  %exitcond.not.i = icmp eq i16 %inc.i, 6
  br i1 %exitcond.not.i, label %for.body.i4, label %for.body.i, !llvm.loop !6

for.body.i4:                                      ; preds = %for.inc.i, %for.inc.i14
  %index.016.i5 = phi i16 [ %inc.i16, %for.inc.i14 ], [ 0, %for.inc.i ]
  %total.015.i6 = phi i32 [ %total.1.i15, %for.inc.i14 ], [ 0, %for.inc.i ]
  %and.i7 = and i16 %index.016.i5, 1
  %cmp1.i8 = icmp eq i16 %and.i7, 0
  br i1 %cmp1.i8, label %if.then.i18, label %if.else.i9

if.then.i18:                                      ; preds = %for.body.i4
  %conv2.i20 = zext nneg i16 %index.016.i5 to i32
  %add.i21 = add nsw i32 %conv2.i20, -3
  %add3.i22 = add i32 %add.i21, %total.015.i6
  br label %for.inc.i14

if.else.i9:                                       ; preds = %for.body.i4
  %narrow24 = add nuw nsw i16 %index.016.i5, 3
  %sub.neg.i12 = zext nneg i16 %narrow24 to i32
  %sub6.i13 = add i32 %total.015.i6, %sub.neg.i12
  br label %for.inc.i14

for.inc.i14:                                      ; preds = %if.else.i9, %if.then.i18
  %total.1.i15 = phi i32 [ %add3.i22, %if.then.i18 ], [ %sub6.i13, %if.else.i9 ]
  %inc.i16 = add nuw nsw i16 %index.016.i5, 1
  %exitcond.not.i17 = icmp eq i16 %inc.i16, 5
  br i1 %exitcond.not.i17, label %parity_control.exit23, label %for.body.i4, !llvm.loop !6

parity_control.exit23:                            ; preds = %for.inc.i14
  %mul = mul nsw i32 %total.1.i, 1000
  %add = add nsw i32 %total.1.i15, %mul
  ret i32 %add
}

attributes #0 = { nofree norecurse nosync nounwind memory(argmem: read) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nofree norecurse nosync nounwind memory(write, argmem: none, inaccessiblemem: none) "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"short", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = distinct !{!6, !7, !8}
!7 = !{!"llvm.loop.mustprogress"}
!8 = !{!"llvm.loop.unroll.disable"}
