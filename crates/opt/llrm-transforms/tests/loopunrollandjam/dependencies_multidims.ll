; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @sub_sub_less(ptr noalias nocapture %A, i32 %N, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp sgt i32 %N, 0
  br i1 %cmp, label %for.outer.preheader, label %cleanup

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.outer.preheader, %for.latch
  %i = phi i32 [ %add7, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 1, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds [100 x i32], ptr %A, i32 %i, i32 %j
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nuw nsw i32 %i, 1
  %add73 = add nuw nsw i32 %j, -1
  %arrayidx8 = getelementptr inbounds [100 x i32], ptr %A, i32 %add72, i32 %add73
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, %N
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, %N
  br i1 %exitcond29, label %cleanup.loopexit, label %for.outer

cleanup.loopexit:                                 ; preds = %for.latch
  br label %cleanup

cleanup:                                          ; preds = %cleanup.loopexit, %entry
  ret void
}

define void @sub_sub_eq(ptr noalias nocapture %A, i32 %N, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp sgt i32 %N, 0
  br i1 %cmp, label %for.outer.preheader, label %cleanup

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.outer.preheader, %for.latch
  %i = phi i32 [ %add7, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds [100 x i32], ptr %A, i32 %i, i32 %j
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nuw nsw i32 %i, 1
  %add73 = add nuw nsw i32 %j, 0
  %arrayidx8 = getelementptr inbounds [100 x i32], ptr %A, i32 %add72, i32 %add73
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, %N
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, %N
  br i1 %exitcond29, label %cleanup.loopexit, label %for.outer

cleanup.loopexit:                                 ; preds = %for.latch
  br label %cleanup

cleanup:                                          ; preds = %cleanup.loopexit, %entry
  ret void
}

define void @sub_sub_more(ptr noalias nocapture %A, i32 %N, ptr noalias readonly nocapture %B) {
entry:
  %cmp = icmp sgt i32 %N, 0
  br i1 %cmp, label %for.outer.preheader, label %cleanup

for.outer.preheader:                              ; preds = %entry
  br label %for.outer

for.outer:                                        ; preds = %for.outer.preheader, %for.latch
  %i = phi i32 [ %add7, %for.latch ], [ 0, %for.outer.preheader ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds [100 x i32], ptr %A, i32 %i, i32 %j
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nuw nsw i32 %i, 1
  %add73 = add nuw nsw i32 %j, 1
  %arrayidx8 = getelementptr inbounds [100 x i32], ptr %A, i32 %add72, i32 %add73
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, %N
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, %N
  br i1 %exitcond29, label %cleanup.loopexit, label %for.outer

cleanup.loopexit:                                 ; preds = %for.latch
  br label %cleanup

cleanup:                                          ; preds = %cleanup.loopexit, %entry
  ret void
}

define void @sub_sub_less_3d(ptr noalias %A) {
entry:
  br label %for.i

for.i:                                            ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %inc.i, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j.latch, %for.i
  %j = phi i32 [ 0, %for.i ], [ %inc.j, %for.j.latch ]
  br label %for.k

for.k:                                            ; preds = %for.k, %for.j
  %k = phi i32 [ 1, %for.j ], [ %inc.k, %for.k ]
  %arrayidx = getelementptr inbounds [100 x [100 x i32]], ptr %A, i32 %i, i32 %j, i32 %k
  store i32 5, ptr %arrayidx, align 4
  %add.i = add nsw i32 %i, 1
  %sub.k = add nsw i32 %k, -1
  %arrayidx2 = getelementptr inbounds [100 x [100 x i32]], ptr %A, i32 %add.i, i32 %j, i32 %sub.k
  store i32 10, ptr %arrayidx2, align 4
  %inc.k = add nsw i32 %k, 1
  %cmp.k = icmp slt i32 %inc.k, 100
  br i1 %cmp.k, label %for.k, label %for.j.latch

for.j.latch:                                      ; preds = %for.k
  %inc.j = add nsw i32 %j, 1
  %cmp.j = icmp slt i32 %inc.j, 100
  br i1 %cmp.j, label %for.j, label %for.i.latch, !llvm.loop !0

for.i.latch:                                      ; preds = %for.j.latch
  %inc.i = add nsw i32 %i, 1
  %cmp.i = icmp slt i32 %inc.i, 100
  br i1 %cmp.i, label %for.i, label %for.end

for.end:                                          ; preds = %for.i.latch
  ret void
}

define void @sub_sub_outer_scalar(ptr %A) {
entry:
  br label %for.i

for.i:                                            ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %inc.i, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j.latch, %for.i
  %j = phi i64 [ 1, %for.i ], [ %inc.j, %for.j.latch ]
  br label %for.k

for.k:                                            ; preds = %for.k, %for.j
  %k = phi i64 [ 0, %for.j ], [ %inc.k, %for.k ]
  %arrayidx = getelementptr inbounds [100 x i32], ptr %A, i64 %j
  %arrayidx7 = getelementptr inbounds [100 x i32], ptr %arrayidx, i64 0, i64 %k
  %0 = load i32, ptr %arrayidx7, align 4
  %sub.j = sub nsw i64 %j, 1
  %arrayidx8 = getelementptr inbounds [100 x i32], ptr %A, i64 %sub.j
  %arrayidx9 = getelementptr inbounds [100 x i32], ptr %arrayidx8, i64 0, i64 %k
  store i32 %0, ptr %arrayidx9, align 4
  %inc.k = add nsw i64 %k, 1
  %cmp.k = icmp slt i64 %inc.k, 100
  br i1 %cmp.k, label %for.k, label %for.j.latch

for.j.latch:                                      ; preds = %for.k
  %inc.j = add nsw i64 %j, 1
  %cmp.j = icmp slt i64 %inc.j, 100
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j.latch
  %inc.i = add nsw i64 %i, 1
  %cmp.i = icmp slt i64 %inc.i, 100
  br i1 %cmp.i, label %for.i, label %for.end

for.end:                                          ; preds = %for.i.latch
  ret void
}

!0 = distinct !{!0, !1}
!1 = !{!"llvm.loop.unroll_and_jam.disable"}
