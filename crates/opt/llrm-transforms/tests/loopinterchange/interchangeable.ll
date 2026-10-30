; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@A = common global [100 x [100 x i64]] zeroinitializer
@B = common global [100 x i64] zeroinitializer

define void @interchange_01(i64 %k, i64 %N) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %j23 = phi i64 [ 0, %entry ], [ %j.next24, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ %j.next, %for2 ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %j23
  %lv = load i64, ptr %arrayidx5, align 8
  %add = add nsw i64 %lv, %k
  store i64 %add, ptr %arrayidx5, align 8
  %j.next = add nuw nsw i64 %j, 1
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc10, label %for2

for1.inc10:                                       ; preds = %for2
  %j.next24 = add nuw nsw i64 %j23, 1
  %exitcond26 = icmp eq i64 %j23, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}

define void @interchange_02(i64 %k) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %j19 = phi i64 [ 0, %entry ], [ %j.next20, %for1.inc10 ]
  br label %for3

for3:                                             ; preds = %for3, %for1.header
  %j = phi i64 [ 100, %for1.header ], [ %j.next, %for3 ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %j19
  %0 = load i64, ptr %arrayidx5, align 8
  %add = add nsw i64 %0, %k
  store i64 %add, ptr %arrayidx5, align 8
  %j.next = add nsw i64 %j, -1
  %cmp2 = icmp sgt i64 %j, 0
  br i1 %cmp2, label %for3, label %for1.inc10

for1.inc10:                                       ; preds = %for3
  %j.next20 = add nuw nsw i64 %j19, 1
  %exitcond = icmp eq i64 %j.next20, 100
  br i1 %exitcond, label %for.end11, label %for1.header

for.end11:                                        ; preds = %for1.inc10
  ret void
}

define void @interchange_10() {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %j23 = phi i64 [ 1, %entry ], [ %j.next24, %for1.inc10 ]
  %j.next24 = add nuw nsw i64 %j23, 1
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i64 [ %j.next, %for2 ], [ 1, %for1.header ]
  %j.next = add nuw nsw i64 %j, 1
  %arrayidx5 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %j23
  store i64 %j, ptr %arrayidx5, align 8
  %arrayidx10 = getelementptr inbounds [100 x [100 x i64]], ptr @A, i64 0, i64 %j, i64 %j.next24
  store i64 %j23, ptr %arrayidx10, align 8
  %exitcond = icmp eq i64 %j, 99
  br i1 %exitcond, label %for1.inc10, label %for2

for1.inc10:                                       ; preds = %for2
  %exitcond26 = icmp eq i64 %j23, 98
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}
