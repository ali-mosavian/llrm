; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

define void @test1(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @nounroll_and_jam(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !0

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @unroll_and_jam_count(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !2

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @unroll_and_jam(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !4

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @nounroll(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !6

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @unroll(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !8

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

define void @nounroll_plus_unroll_and_jam(i32 %I, i32 %J, ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp ne i32 %J, 0
  %cmp122 = icmp ne i32 %I, 0
  %or.cond = and i1 %cmp, %cmp122
  br i1 %or.cond, label %for.outer.preheader, label %for.end

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %for.outer.preheader
  %i.us = phi i32 [ %add8.us, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j.us = phi i32 [ 0, %for.outer ], [ %inc.us, %for.inner ]
  %sum1.us = phi i32 [ 0, %for.outer ], [ %add.us, %for.inner ]
  %arrayidx.us = getelementptr inbounds i32, ptr %B, i32 %j.us
  %0 = load i32, ptr %arrayidx.us, align 4
  %add.us = add i32 %0, %sum1.us
  %inc.us = add nuw i32 %j.us, 1
  %exitcond = icmp eq i32 %inc.us, %J
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.us.lcssa = phi i32 [ %add.us, %for.inner ]
  %arrayidx6.us = getelementptr inbounds i32, ptr %A, i32 %i.us
  store i32 %add.us.lcssa, ptr %arrayidx6.us, align 4
  %add8.us = add nuw i32 %i.us, 1
  %exitcond25 = icmp eq i32 %add8.us, %I
  br i1 %exitcond25, label %for.end.loopexit, label %for.outer, !llvm.loop !10

for.end.loopexit:                                 ; preds = %for.latch
  br label %for.end

for.end:                                          ; preds = %for.end.loopexit, %entry
  ret void
}

!0 = distinct !{!0, !1}
!1 = distinct !{!"llvm.loop.unroll_and_jam.disable"}
!2 = distinct !{!2, !3}
!3 = distinct !{!"llvm.loop.unroll_and_jam.count", i32 8}
!4 = distinct !{!4, !5}
!5 = distinct !{!"llvm.loop.unroll_and_jam.enable"}
!6 = distinct !{!6, !7}
!7 = distinct !{!"llvm.loop.unroll.disable"}
!8 = distinct !{!8, !9}
!9 = distinct !{!"llvm.loop.unroll.enable"}
!10 = distinct !{!10, !7, !5}
