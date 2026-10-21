; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@A = common global [100 x [100 x i32]] zeroinitializer
@B = common global [100 x [100 x [100 x i32]]] zeroinitializer
@C = common global [100 x [100 x i64]] zeroinitializer

define void @interchange_01(i32 %k, i32 %N) {
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

define void @interchange_02(i64 %k, i64 %N) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %j23 = phi i64 [ 0, %entry ], [ %j.next24, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %latch, %for1.header
  %j = phi i64 [ %j.next, %latch ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @C, i64 0, i64 %j, i64 %j23
  %lv = load i64, ptr %arrayidx5, align 8
  %add = add nsw i64 %lv, %k
  store i64 %add, ptr %arrayidx5, align 8
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc10, label %latch

latch:                                            ; preds = %for2
  %j.next = add nuw nsw i64 %j, 1
  br label %for2

for1.inc10:                                       ; preds = %for2
  %j.next24 = add nuw nsw i64 %j23, 1
  %exitcond26 = icmp eq i64 %j23, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}
