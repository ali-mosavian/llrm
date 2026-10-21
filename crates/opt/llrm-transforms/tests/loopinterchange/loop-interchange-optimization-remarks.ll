; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = common global [100 x [100 x i32]] zeroinitializer
@B = common global [100 x [100 x i32]] zeroinitializer
@C = common global [100 x i32] zeroinitializer
@A10 = local_unnamed_addr global [3 x [3 x i32]] zeroinitializer, align 16

define void @test01(i32 %N) {
entry:
  %cmp31 = icmp sgt i32 %N, 1
  br i1 %cmp31, label %for.cond1.preheader.lr.ph, label %for.end19

for.cond1.preheader.lr.ph:                        ; preds = %entry
  %0 = add i32 %N, -1
  br label %for.body3.lr.ph

for.body3.lr.ph:                                  ; preds = %for.inc17, %for.cond1.preheader.lr.ph
  %indvars.iv34 = phi i64 [ 1, %for.cond1.preheader.lr.ph ], [ %indvars.iv.next35, %for.inc17 ]
  %1 = add nsw i64 %indvars.iv34, -1
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body3.lr.ph
  %indvars.iv = phi i64 [ 1, %for.body3.lr.ph ], [ %indvars.iv.next, %for.body3 ]
  %2 = add nsw i64 %indvars.iv, -1
  %arrayidx6 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %1, i64 %2
  %3 = load i32, ptr %arrayidx6, align 4
  %arrayidx10 = getelementptr inbounds [100 x [100 x i32]], ptr @B, i64 0, i64 %indvars.iv34, i64 %indvars.iv
  %4 = load i32, ptr %arrayidx10, align 4
  %add = add nsw i32 %4, %3
  store i32 %add, ptr %arrayidx6, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %lftr.wideiv = trunc i64 %indvars.iv to i32
  %exitcond = icmp eq i32 %lftr.wideiv, %0
  br i1 %exitcond, label %for.inc17, label %for.body3

for.inc17:                                        ; preds = %for.body3
  %indvars.iv.next35 = add nuw nsw i64 %indvars.iv34, 1
  %lftr.wideiv37 = trunc i64 %indvars.iv34 to i32
  %exitcond38 = icmp eq i32 %lftr.wideiv37, %0
  br i1 %exitcond38, label %for.end19.loopexit, label %for.body3.lr.ph

for.end19.loopexit:                               ; preds = %for.inc17
  br label %for.end19

for.end19:                                        ; preds = %for.end19.loopexit, %entry
  ret void
}

define void @test02(i32 %k, i32 %N) {
entry:
  %sub = add nsw i32 %N, -1
  %cmp26 = icmp sgt i32 %N, 1
  br i1 %cmp26, label %for.cond1.preheader.lr.ph, label %for.end17

for.cond1.preheader.lr.ph:                        ; preds = %entry
  %cmp324 = icmp sgt i32 %sub, 1
  %0 = add i32 %N, -2
  %1 = sext i32 %sub to i64
  br label %for.cond1.preheader

for.cond.loopexit.loopexit:                       ; preds = %for.body4
  br label %for.cond.loopexit

for.cond.loopexit:                                ; preds = %for.cond.loopexit.loopexit, %for.cond1.preheader
  %cmp = icmp slt i64 %indvars.iv.next29, %1
  br i1 %cmp, label %for.cond1.preheader, label %for.end17.loopexit

for.cond1.preheader:                              ; preds = %for.cond.loopexit, %for.cond1.preheader.lr.ph
  %indvars.iv28 = phi i64 [ 0, %for.cond1.preheader.lr.ph ], [ %indvars.iv.next29, %for.cond.loopexit ]
  %indvars.iv.next29 = add nuw nsw i64 %indvars.iv28, 1
  br i1 %cmp324, label %for.body4.preheader, label %for.cond.loopexit

for.body4.preheader:                              ; preds = %for.cond1.preheader
  br label %for.body4

for.body4:                                        ; preds = %for.body4.preheader, %for.body4
  %indvars.iv = phi i64 [ %indvars.iv.next, %for.body4 ], [ 1, %for.body4.preheader ]
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %arrayidx7 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %indvars.iv.next, i64 %indvars.iv.next29
  %2 = load i32, ptr %arrayidx7, align 4
  %add8 = add nsw i32 %2, %k
  store i32 %add8, ptr %arrayidx7, align 4
  %lftr.wideiv = trunc i64 %indvars.iv to i32
  %exitcond = icmp eq i32 %lftr.wideiv, %0
  br i1 %exitcond, label %for.cond.loopexit.loopexit, label %for.body4

for.end17.loopexit:                               ; preds = %for.cond.loopexit
  br label %for.end17

for.end17:                                        ; preds = %for.end17.loopexit, %entry
  ret void
}

define void @test03() {
entry:
  br label %for.cond1.preheader

for.cond.loopexit:                                ; preds = %for.body4
  %exitcond28 = icmp ne i64 %indvars.iv.next27, 2
  br i1 %exitcond28, label %for.cond1.preheader, label %for.cond.cleanup

for.cond1.preheader:                              ; preds = %for.cond.loopexit, %entry
  %indvars.iv26 = phi i64 [ 0, %entry ], [ %indvars.iv.next27, %for.cond.loopexit ]
  %indvars.iv.next27 = add nuw nsw i64 %indvars.iv26, 1
  br label %for.body4

for.cond.cleanup:                                 ; preds = %for.cond.loopexit
  ret void

for.body4:                                        ; preds = %for.body4, %for.cond1.preheader
  %indvars.iv = phi i64 [ 0, %for.cond1.preheader ], [ %indvars.iv.next, %for.body4 ]
  %arrayidx6 = getelementptr inbounds [3 x [3 x i32]], ptr @A10, i64 0, i64 %indvars.iv, i64 %indvars.iv26
  %tmp = trunc i64 %indvars.iv26 to i32
  store i32 %tmp, ptr %arrayidx6, align 4
  %arrayidx10 = getelementptr inbounds [3 x [3 x i32]], ptr @A10, i64 0, i64 %indvars.iv, i64 %indvars.iv.next27
  %tmp1 = trunc i64 %indvars.iv to i32
  store i32 %tmp1, ptr %arrayidx10, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 3
  br i1 %exitcond, label %for.body4, label %for.cond.loopexit
}

define void @test04(i32 %k, i32 %N) {
entry:
  %cmp30 = icmp sgt i32 %N, 0
  br i1 %cmp30, label %for.body.lr.ph, label %for.end17

for.body.lr.ph:                                   ; preds = %entry
  %0 = add i32 %N, -1
  %1 = zext i32 %k to i64
  br label %for.body

for.body:                                         ; preds = %for.inc15, %for.body.lr.ph
  %indvars.iv32 = phi i64 [ 0, %for.body.lr.ph ], [ %indvars.iv.next33, %for.inc15 ]
  %2 = add nsw i64 %indvars.iv32, %1
  %arrayidx = getelementptr inbounds [100 x i32], ptr @C, i64 0, i64 %indvars.iv32
  %3 = trunc i64 %2 to i32
  store i32 %3, ptr %arrayidx, align 4
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body
  %indvars.iv = phi i64 [ 0, %for.body ], [ %indvars.iv.next, %for.body3 ]
  %arrayidx7 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %indvars.iv32, i64 %indvars.iv
  %4 = load i32, ptr %arrayidx7, align 4
  %add10 = add nsw i32 %3, %4
  store i32 %add10, ptr %arrayidx7, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %lftr.wideiv = trunc i64 %indvars.iv to i32
  %exitcond = icmp eq i32 %lftr.wideiv, %0
  br i1 %exitcond, label %for.inc15, label %for.body3

for.inc15:                                        ; preds = %for.body3
  %indvars.iv.next33 = add nuw nsw i64 %indvars.iv32, 1
  %lftr.wideiv35 = trunc i64 %indvars.iv32 to i32
  %exitcond36 = icmp eq i32 %lftr.wideiv35, %0
  br i1 %exitcond36, label %for.end17.loopexit, label %for.body

for.end17.loopexit:                               ; preds = %for.inc15
  br label %for.end17

for.end17:                                        ; preds = %for.end17.loopexit, %entry
  ret void
}
