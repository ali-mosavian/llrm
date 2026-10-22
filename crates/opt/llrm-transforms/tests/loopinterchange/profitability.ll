; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@A = common global [100 x [100 x i32]] zeroinitializer
@B = common global [100 x [100 x i32]] zeroinitializer

define void @interchange_01() {
entry:
  br label %for2.preheader

for2.preheader:                                   ; preds = %for1.inc14, %entry
  %i30 = phi i64 [ 1, %entry ], [ %i.next31, %for1.inc14 ]
  br label %for2

for2:                                             ; preds = %for2, %for2.preheader
  %j = phi i64 [ %i.next, %for2 ], [ 1, %for2.preheader ]
  %j.prev = add nsw i64 %j, -1
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j.prev, i64 %i30
  %lv1 = load i32, ptr %arrayidx5, align 4
  %arrayidx9 = getelementptr inbounds [100 x [100 x i32]], ptr @B, i64 0, i64 %j, i64 %i30
  %lv2 = load i32, ptr %arrayidx9, align 4
  %add = add nsw i32 %lv1, %lv2
  %arrayidx13 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j, i64 %i30
  store i32 %add, ptr %arrayidx13, align 4
  %i.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc14, label %for2

for1.inc14:                                       ; preds = %for2
  %i.next31 = add nuw nsw i64 %i30, 1
  %exitcond33 = icmp eq i64 %i30, 99
  br i1 %exitcond33, label %for.end16, label %for2.preheader

for.end16:                                        ; preds = %for1.inc14
  ret void
}

define void @interchange_02() {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc19, %entry
  %i35 = phi i64 [ 1, %entry ], [ %i.next36, %for1.inc19 ]
  %i.prev = add nsw i64 %i35, -1
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ 1, %for1.header ], [ %i.next, %for2 ]
  %j.prev = add nsw i64 %j, -1
  %arrayidx6 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j.prev, i64 %i.prev
  %lv1 = load i32, ptr %arrayidx6, align 4
  %arrayidx12 = getelementptr inbounds [100 x [100 x i32]], ptr @B, i64 0, i64 %j.prev, i64 %i.prev
  %lv2 = load i32, ptr %arrayidx12, align 4
  %add = add nsw i32 %lv1, %lv2
  store i32 %add, ptr %arrayidx6, align 4
  %i.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc19, label %for2

for1.inc19:                                       ; preds = %for2
  %i.next36 = add nuw nsw i64 %i35, 1
  %exitcond39 = icmp eq i64 %i35, 99
  br i1 %exitcond39, label %for.end21, label %for1.header

for.end21:                                        ; preds = %for1.inc19
  ret void
}

define void @interchange_03() {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc17, %entry
  %i34 = phi i64 [ 1, %entry ], [ %i.next35, %for1.inc17 ]
  %i.prev = add nsw i64 %i34, -1
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ 1, %for1.header ], [ %i.next, %for2 ]
  %j.prev = add nsw i64 %j, -1
  %arrayidx6 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %i.prev, i64 %j.prev
  %lv1 = load i32, ptr %arrayidx6, align 4
  %arrayidx10 = getelementptr inbounds [100 x [100 x i32]], ptr @B, i64 0, i64 %i34, i64 %j
  %lv2 = load i32, ptr %arrayidx10, align 4
  %add = add nsw i32 %lv1, %lv2
  store i32 %add, ptr %arrayidx6, align 4
  %i.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc17, label %for2

for1.inc17:                                       ; preds = %for2
  %i.next35 = add nuw nsw i64 %i34, 1
  %exitcond38 = icmp eq i64 %i34, 99
  br i1 %exitcond38, label %for.end19, label %for1.header

for.end19:                                        ; preds = %for1.inc17
  ret void
}

define void @interchange_04(i32 %k) {
entry:
  br label %for.cond1.preheader

for.cond1.preheader:                              ; preds = %for.inc10, %entry
  %indvars.iv21 = phi i64 [ 0, %entry ], [ %indvars.iv.next22, %for.inc10 ]
  br label %for.body3

for.body3:                                        ; preds = %for.body3, %for.cond1.preheader
  %indvars.iv = phi i64 [ 0, %for.cond1.preheader ], [ %indvars.iv.next, %for.body3 ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %indvars.iv21, i64 %indvars.iv
  %0 = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %0, %k
  store i32 %add, ptr %arrayidx5, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp eq i64 %indvars.iv.next, 100
  br i1 %exitcond, label %for.inc10, label %for.body3

for.inc10:                                        ; preds = %for.body3
  %indvars.iv.next22 = add nuw nsw i64 %indvars.iv21, 1
  %exitcond23 = icmp eq i64 %indvars.iv.next22, 100
  br i1 %exitcond23, label %for.end12, label %for.cond1.preheader

for.end12:                                        ; preds = %for.inc10
  ret void
}

define void @interchange_05() {
entry:
  br label %for2.preheader

for2.preheader:                                   ; preds = %for1.inc14, %entry
  %i30 = phi i64 [ 1, %entry ], [ %i.next31, %for1.inc14 ]
  br label %for2

for2:                                             ; preds = %for2, %for2.preheader
  %j = phi i64 [ %i.next, %for2 ], [ 1, %for2.preheader ]
  %j.prev = add nsw i64 %j, -1
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j, i64 0
  %lv1 = load i32, ptr %arrayidx5, align 4
  %arrayidx9 = getelementptr inbounds [100 x [100 x i32]], ptr @B, i64 0, i64 %j, i64 %i30
  %lv2 = load i32, ptr %arrayidx9, align 4
  %add = add nsw i32 %lv1, %lv2
  %arrayidx13 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %j, i64 0
  store i32 %add, ptr %arrayidx13, align 4
  %i.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc14, label %for2

for1.inc14:                                       ; preds = %for2
  %i.next31 = add nuw nsw i64 %i30, 1
  %exitcond33 = icmp eq i64 %i30, 99
  br i1 %exitcond33, label %for.end16, label %for2.preheader

for.end16:                                        ; preds = %for1.inc14
  ret void
}
