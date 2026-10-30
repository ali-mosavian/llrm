; ModuleID = '<stdin>'
source_filename = "<stdin>"

@A = common global [100 x [100 x i64]] zeroinitializer
@N = local_unnamed_addr global i64 100, align 8

define void @interchange_01(i64 %k) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ %j.next, %for2 ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %i
  %lv = load i64, ptr %arrayidx5, align 4
  %add = add nsw i64 %lv, %k
  store i64 %add, ptr %arrayidx5, align 4
  %j.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, %i
  br i1 %exitcond, label %for1.inc10, label %for2

for1.inc10:                                       ; preds = %for2
  %i.next = add nuw nsw i64 %i, 1
  %exitcond26 = icmp eq i64 %i, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}

define void @interchange_02(i64 %k) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ %j.next, %for2 ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %i
  %lv = load i64, ptr %arrayidx5, align 4
  %add = add nsw i64 %lv, %k
  store i64 %add, ptr %arrayidx5, align 4
  %0 = add nuw nsw i64 %j, %i
  %j.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %0, 100
  br i1 %exitcond, label %for1.inc10, label %for2

for1.inc10:                                       ; preds = %for2
  %i.next = add nuw nsw i64 %i, 1
  %exitcond26 = icmp eq i64 %i, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}

define void @interchange_03(i64 %k) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ %j.next, %for2 ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %i
  %lv = load i64, ptr %arrayidx5, align 4
  %add = add nsw i64 %lv, %k
  store i64 %add, ptr %arrayidx5, align 4
  %j.next = add nuw nsw i64 %j, 1
  %exitcond = icmp ne i64 %i, %j
  br i1 %exitcond, label %for2, label %for1.inc10

for1.inc10:                                       ; preds = %for2
  %i.next = add nuw nsw i64 %i, 1
  %exitcond26 = icmp eq i64 %i, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}
