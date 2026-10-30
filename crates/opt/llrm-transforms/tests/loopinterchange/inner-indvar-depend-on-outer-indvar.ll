; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-p:32:32"

@A = common global [100 x [100 x i32]] zeroinitializer
@N = local_unnamed_addr global i32 100, align 8

define void @interchange_04(i32 %k) {
entry:
  %0 = load i32, ptr @N, align 4
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %i = phi i32 [ 0, %entry ], [ %i.next, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %j = phi i32 [ %j.next, %for2 ], [ 0, %for1.header ]
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i32 0, i32 %j, i32 %i
  %lv = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %lv, %k
  store i32 %add, ptr %arrayidx5, align 4
  %j.next = add nuw nsw i32 %j, 1
  %exitcond = icmp ne i32 %0, %j
  br i1 %exitcond, label %for2, label %for1.inc10

for1.inc10:                                       ; preds = %for2
  %i.next = add nuw nsw i32 %i, 1
  %exitcond26 = icmp eq i32 %i, 99
  br i1 %exitcond26, label %for.end12, label %for1.header

for.end12:                                        ; preds = %for1.inc10
  ret void
}
