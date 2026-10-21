; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @issue46867(ptr noundef nocapture %s, i32 noundef %c, ptr noundef readonly nocapture %ff) {
entry:
  %tobool7.not = icmp eq i32 %c, 0
  br i1 %tobool7.not, label %for.cond1.preheader.us.preheader, label %entry.split

for.cond1.preheader.us.preheader:                 ; preds = %entry
  br label %for.cond1.preheader.us

for.cond1.preheader.us:                           ; preds = %for.cond.cleanup3.split.us.us, %for.cond1.preheader.us.preheader
  %indvars.iv31 = phi i64 [ 0, %for.cond1.preheader.us.preheader ], [ %indvars.iv.next32, %for.cond.cleanup3.split.us.us ]
  br label %for.body4.us.us

for.body4.us.us:                                  ; preds = %land.end.us.us, %for.cond1.preheader.us
  %indvars.iv27 = phi i64 [ %indvars.iv.next28, %land.end.us.us ], [ 0, %for.cond1.preheader.us ]
  %arrayidx6.us.us = getelementptr inbounds nuw [9 x i32], ptr %ff, i64 %indvars.iv27, i64 %indvars.iv31
  %0 = load i32, ptr %arrayidx6.us.us, align 4
  %tobool.not.us.us = icmp eq i32 %0, 0
  br i1 %tobool.not.us.us, label %land.end.us.us, label %land.rhs.us.us

land.rhs.us.us:                                   ; preds = %for.body4.us.us
  store i32 3, ptr %s, align 4
  br label %land.end.us.us

land.end.us.us:                                   ; preds = %land.rhs.us.us, %for.body4.us.us
  %indvars.iv.next28 = add nuw nsw i64 %indvars.iv27, 1
  %exitcond30 = icmp ne i64 %indvars.iv.next28, 3
  br i1 %exitcond30, label %for.body4.us.us, label %for.cond.cleanup3.split.us.us

for.cond.cleanup3.split.us.us:                    ; preds = %land.end.us.us
  %indvars.iv.next32 = add nuw nsw i64 %indvars.iv31, 1
  %exitcond34 = icmp ne i64 %indvars.iv.next32, 3
  br i1 %exitcond34, label %for.cond1.preheader.us, label %for.cond.cleanup.loopexit

entry.split:                                      ; preds = %entry
  %s.promoted19 = load i32, ptr %s, align 4
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup3.split, %entry.split
  %indvars.iv23 = phi i64 [ 0, %entry.split ], [ %indvars.iv.next24, %for.cond.cleanup3.split ]
  %s.promoted20 = phi i32 [ %s.promoted19, %entry.split ], [ %inc.lcssa, %for.cond.cleanup3.split ]
  br label %for.body4

for.cond.cleanup.loopexit:                        ; preds = %for.cond.cleanup3.split.us.us
  br label %for.cond.cleanup

for.cond.cleanup.loopexit21:                      ; preds = %for.cond.cleanup3.split
  br label %for.cond.cleanup

for.cond.cleanup:                                 ; preds = %for.cond.cleanup.loopexit21, %for.cond.cleanup.loopexit
  ret void

for.cond.cleanup3.split:                          ; preds = %land.end
  %inc.lcssa = phi i32 [ %inc, %land.end ]
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exitcond26 = icmp ne i64 %indvars.iv.next24, 3
  br i1 %exitcond26, label %for.cond1.preheader, label %for.cond.cleanup.loopexit21

for.body4:                                        ; preds = %land.end, %for.cond1.preheader
  %indvars.iv = phi i64 [ 0, %for.cond1.preheader ], [ %indvars.iv.next, %land.end ]
  %1 = phi i32 [ %s.promoted20, %for.cond1.preheader ], [ %inc, %land.end ]
  %arrayidx6 = getelementptr inbounds nuw [9 x i32], ptr %ff, i64 %indvars.iv, i64 %indvars.iv23
  %2 = load i32, ptr %arrayidx6, align 4
  %tobool.not = icmp eq i32 %2, 0
  br i1 %tobool.not, label %land.end, label %land.rhs

land.rhs:                                         ; preds = %for.body4
  store i32 3, ptr %s, align 4
  br label %land.end

land.end:                                         ; preds = %land.rhs, %for.body4
  %3 = phi i32 [ 3, %land.rhs ], [ %1, %for.body4 ]
  %inc = add nsw i32 %3, 1
  store i32 %inc, ptr %s, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 3
  br i1 %exitcond, label %for.body4, label %for.cond.cleanup3.split
}

define void @issue47401(ptr noundef writeonly nocapture %e, ptr noundef readonly nocapture %bb) {
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup2, %entry
  %indvars.iv22 = phi i64 [ 0, %entry ], [ %indvars.iv.next23, %for.cond.cleanup2 ]
  br label %for.body3

for.cond.cleanup:                                 ; preds = %for.cond.cleanup2
  ret void

for.cond.cleanup2:                                ; preds = %land.end
  %indvars.iv.next23 = add nuw nsw i64 %indvars.iv22, 1
  %exitcond = icmp ne i64 %indvars.iv.next23, 8
  br i1 %exitcond, label %for.cond1.preheader, label %for.cond.cleanup

for.body3:                                        ; preds = %land.end, %for.cond1.preheader
  %indvars.iv = phi i64 [ 4, %for.cond1.preheader ], [ %indvars.iv.next, %land.end ]
  %0 = getelementptr [8 x i32], ptr %bb, i64 %indvars.iv
  %arrayidx = getelementptr i8, ptr %0, i64 64
  %arrayidx5 = getelementptr inbounds nuw [8 x i32], ptr %arrayidx, i64 0, i64 %indvars.iv22
  %1 = load i32, ptr %arrayidx5, align 4
  %tobool6.not = icmp eq i32 %1, 0
  br i1 %tobool6.not, label %land.end, label %land.rhs

land.rhs:                                         ; preds = %for.body3
  %2 = load i32, ptr %0, align 4
  store i32 %2, ptr %e, align 4
  br label %land.end

land.end:                                         ; preds = %land.rhs, %for.body3
  %indvars.iv.next = add nsw i64 %indvars.iv, -1
  %tobool.not = icmp eq i64 %indvars.iv.next, 0
  br i1 %tobool.not, label %for.cond.cleanup2, label %for.body3
}

define void @issue47295(ptr noundef nocapture %f, ptr noundef writeonly nocapture %cc) {
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup3, %entry
  %indvars.iv18 = phi i64 [ 0, %entry ], [ %indvars.iv.next19, %for.cond.cleanup3 ]
  br label %for.body4

for.cond.cleanup:                                 ; preds = %for.cond.cleanup3
  ret void

for.cond.cleanup3:                                ; preds = %for.body4
  %indvars.iv.next19 = add nuw nsw i64 %indvars.iv18, 1
  %exitcond21 = icmp ne i64 %indvars.iv.next19, 4
  br i1 %exitcond21, label %for.cond1.preheader, label %for.cond.cleanup

for.body4:                                        ; preds = %for.body4, %for.cond1.preheader
  %indvars.iv = phi i64 [ 0, %for.cond1.preheader ], [ %indvars.iv.next, %for.body4 ]
  %0 = load i32, ptr %f, align 4
  %xor = xor i32 %0, 4096
  store i32 %xor, ptr %f, align 4
  %arrayidx6 = getelementptr inbounds nuw [4 x i32], ptr %cc, i64 %indvars.iv, i64 %indvars.iv18
  store i32 %xor, ptr %arrayidx6, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 4
  br i1 %exitcond, label %for.body4, label %for.cond.cleanup3
}

define void @issue54176(i32 noundef %n, i32 noundef %m, ptr noundef nocapture %aa, ptr noundef readonly nocapture %bb, ptr noundef writeonly nocapture %cc) {
entry:
  %arrayidx = getelementptr inbounds nuw i8, ptr %aa, i64 512
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.cond.cleanup3, %entry
  %indvars.iv32 = phi i64 [ 1, %entry ], [ %indvars.iv.next33, %for.cond.cleanup3 ]
  %arrayidx5 = getelementptr inbounds nuw [128 x float], ptr %arrayidx, i64 0, i64 %indvars.iv32
  %0 = add nsw i64 %indvars.iv32, -1
  %arrayidx16 = getelementptr inbounds [128 x float], ptr %arrayidx, i64 0, i64 %0
  br label %for.body4

for.cond.cleanup3:                                ; preds = %for.body4
  %indvars.iv.next33 = add nuw nsw i64 %indvars.iv32, 1
  %exitcond36 = icmp ne i64 %indvars.iv.next33, 128
  br i1 %exitcond36, label %for.cond1.preheader, label %for.cond.cleanup

for.body4:                                        ; preds = %for.body4, %for.cond1.preheader
  %indvars.iv = phi i64 [ 1, %for.cond1.preheader ], [ %indvars.iv.next, %for.body4 ]
  %1 = load float, ptr %arrayidx5, align 4
  %arrayidx9 = getelementptr inbounds nuw [128 x float], ptr %cc, i64 %indvars.iv, i64 %indvars.iv32
  store float %1, ptr %arrayidx9, align 4
  %arrayidx13 = getelementptr inbounds nuw [128 x float], ptr %bb, i64 %indvars.iv, i64 %indvars.iv32
  %2 = load float, ptr %arrayidx13, align 4
  %3 = load float, ptr %arrayidx16, align 4
  %add = fadd float %2, %3
  store float %add, ptr %arrayidx16, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 1024
  br i1 %exitcond, label %for.body4, label %for.cond.cleanup3

for.cond.cleanup:                                 ; preds = %for.cond.cleanup3
  ret void
}

define void @issue116114(ptr noundef nocapture %A, i32 noundef %x, i32 noundef %N, i32 noundef %M) {
entry:
  %cmp18.not = icmp eq i32 %M, 0
  br i1 %cmp18.not, label %for.cond.cleanup, label %for.cond1.preheader.lr.ph

for.cond1.preheader.lr.ph:                        ; preds = %entry
  %sub = add i32 %N, -1
  %cmp216.not = icmp eq i32 %sub, 0
  br i1 %cmp216.not, label %for.cond1.preheader.preheader, label %for.cond1.preheader.us.preheader

for.cond1.preheader.us.preheader:                 ; preds = %for.cond1.preheader.lr.ph
  br label %for.cond1.preheader.us

for.cond1.preheader.preheader:                    ; preds = %for.cond1.preheader.lr.ph
  br label %for.cond.cleanup.loopexit

for.cond1.preheader.us:                           ; preds = %for.cond1.for.cond.cleanup3_crit_edge.us, %for.cond1.preheader.us.preheader
  %m.019.us = phi i32 [ %inc9.us, %for.cond1.for.cond.cleanup3_crit_edge.us ], [ 0, %for.cond1.preheader.us.preheader ]
  %wide.trip.count = zext i32 %sub to i64
  br label %for.body4.us

for.body4.us:                                     ; preds = %for.body4.us, %for.cond1.preheader.us
  %indvars.iv = phi i64 [ 0, %for.cond1.preheader.us ], [ %indvars.iv.next, %for.body4.us ]
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %arrayidx.us = getelementptr inbounds nuw i32, ptr %A, i64 %indvars.iv.next
  %0 = load i32, ptr %arrayidx.us, align 4
  %add5.us = add nsw i32 %0, %x
  %arrayidx7.us = getelementptr inbounds nuw i32, ptr %A, i64 %indvars.iv
  store i32 %add5.us, ptr %arrayidx7.us, align 4
  %exitcond = icmp ne i64 %indvars.iv.next, %wide.trip.count
  br i1 %exitcond, label %for.body4.us, label %for.cond1.for.cond.cleanup3_crit_edge.us

for.cond1.for.cond.cleanup3_crit_edge.us:         ; preds = %for.body4.us
  %inc9.us = add nuw i32 %m.019.us, 1
  %exitcond22 = icmp ne i32 %inc9.us, %M
  br i1 %exitcond22, label %for.cond1.preheader.us, label %for.cond.cleanup.loopexit20

for.cond.cleanup.loopexit:                        ; preds = %for.cond1.preheader.preheader
  br label %for.cond.cleanup

for.cond.cleanup.loopexit20:                      ; preds = %for.cond1.for.cond.cleanup3_crit_edge.us
  br label %for.cond.cleanup

for.cond.cleanup:                                 ; preds = %for.cond.cleanup.loopexit20, %for.cond.cleanup.loopexit, %entry
  ret void
}
