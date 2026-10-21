; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-i64:64-f80:128-n8:16:32:64-S128"

@A = common global [100 x [100 x i32]] zeroinitializer

declare void @foo(i64)

; Function Attrs: nounwind willreturn memory(none)
declare void @bar(i64) #0

define void @interchange_02(i32 %k) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc10, %entry
  %indvars.iv23 = phi i64 [ 0, %entry ], [ %indvars.iv.next24, %for1.inc10 ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ %indvars.iv.next, %for2 ], [ 1, %for1.header ]
  call void @bar(i64 %indvars.iv23)
  %arrayidx5 = getelementptr inbounds [100 x [100 x i32]], ptr @A, i64 0, i64 %indvars.iv, i64 %indvars.iv23
  %lv = load i32, ptr %arrayidx5, align 4
  %add = add nsw i32 %lv, %k
  store i32 %add, ptr %arrayidx5, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp eq i64 %indvars.iv, 99
  br i1 %exitcond, label %for2.loopexit, label %for2

for2.loopexit:                                    ; preds = %for2
  br label %for1.inc10

for1.inc10:                                       ; preds = %for2.loopexit
  %indvars.iv.next24 = add nuw nsw i64 %indvars.iv23, 1
  %exitcond26 = icmp eq i64 %indvars.iv23, 99
  br i1 %exitcond26, label %for1.loopexit, label %for1.header

for1.loopexit:                                    ; preds = %for1.inc10
  br label %exit

exit:                                             ; preds = %for1.loopexit
  ret void
}

attributes #0 = { nounwind willreturn memory(none) }
