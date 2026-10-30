; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@A = common global [100 x [100 x i32]] zeroinitializer
@B = common global [100 x i32] zeroinitializer
@C = common global [100 x [100 x i32]] zeroinitializer
@D = common global [100 x [100 x [100 x i32]]] zeroinitializer

define void @interchange_05(i32 %k, i32 %N) {
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
  %arrayidx = getelementptr inbounds [100 x i32], ptr @B, i64 0, i64 %indvars.iv32
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

; Function Attrs: memory(none)
declare void @foo(...) #0

define void @interchange_06(i32 %k, i32 %N) {
entry:
  %cmp22 = icmp sgt i32 %N, 0
  br i1 %cmp22, label %for.body.lr.ph, label %for.end12

for.body.lr.ph:                                   ; preds = %entry
  %0 = add i32 %N, -1
  br label %for.body

for.body:                                         ; preds = %for.inc10, %for.body.lr.ph
  %indvars.iv24 = phi i64 [ 0, %for.body.lr.ph ], [ %indvars.iv.next25, %for.inc10 ]
  tail call void (...) @foo()
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.body
  %indvars.iv = phi i64 [ %indvars.iv.next, %for.body3 ], [ 2, %for.body ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %indvars.iv24, i64 %indvars.iv
  %1 = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %1, %k
  store i32 %add, ptr %arrayidx5, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %lftr.wideiv = trunc i64 %indvars.iv to i32
  %exitcond = icmp eq i32 %lftr.wideiv, %0
  br i1 %exitcond, label %for.inc10, label %for.body3

for.inc10:                                        ; preds = %for.body3
  %indvars.iv.next25 = add nuw nsw i64 %indvars.iv24, 1
  %lftr.wideiv26 = trunc i64 %indvars.iv24 to i32
  %exitcond27 = icmp eq i32 %lftr.wideiv26, %0
  br i1 %exitcond27, label %for.end12.loopexit, label %for.body

for.end12.loopexit:                               ; preds = %for.inc10
  br label %for.end12

for.end12:                                        ; preds = %for.end12.loopexit, %entry
  ret void
}

define void @interchange_07(i32 %k, i32 %N, i64 %ny) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %j23 = phi i64 [ 0, %entry ], [ %j.next24, %for1.inc10 ]
  %cmp21 = icmp slt i64 0, %ny
  br label %singleSucc

singleSucc:                                       ; preds = %for1.header
  br i1 %cmp21, label %preheader.j, label %for1.inc10

preheader.j:                                      ; preds = %singleSucc
  br label %for2

for2:                                             ; preds = %for2, %preheader.j
  %j = phi i64 [ %j.next, %for2 ], [ 0, %preheader.j ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j, i64 %j23
  %lv = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %lv, %k
  store i32 %add, ptr %arrayidx5, align 4
  %j.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc10.loopexit, label %for2

for1.inc10.loopexit:                              ; preds = %for2
  br label %for1.inc10

for1.inc10:                                       ; preds = %for1.inc10.loopexit, %singleSucc
  %j.next24 = add nuw nsw i64 %j23, 1
  %exitcond26 = icmp eq i64 %j23, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}

attributes #0 = { memory(none) }
