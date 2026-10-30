; ModuleID = '<stdin>'
source_filename = "<stdin>"
target datalayout = "e-m:e-p:32:32-i64:64-v128:64:128-a:0:32-n32-S64"

define void @fore_aft_less(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nsw i32 %i, -1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @fore_aft_eq(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nuw nsw i32 %i, 0
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @fore_aft_more(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nuw nsw i32 %i, 1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @fore_sub_less(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add72 = add nsw i32 %i, -1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @fore_sub_eq(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add72 = add nuw nsw i32 %i, 0
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @fore_sub_more(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add72 = add nuw nsw i32 %i, 1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %add6 = add nuw nsw i32 %j, 1
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_aft_less(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nsw i32 %i, -1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_aft_eq(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nuw nsw i32 %i, 0
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_aft_more(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add.lcssa = phi i32 [ %add, %for.inner ]
  %add7 = add nuw nsw i32 %i, 1
  %add72 = add nuw nsw i32 %i, 1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add.lcssa, ptr %arrayidx8, align 4
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_sub_less(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nsw i32 %i, -1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_sub_eq(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nuw nsw i32 %i, 0
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}

define void @sub_sub_more(ptr noalias nocapture %A, ptr noalias readonly nocapture %B) {
entry:
  br label %for.outer

for.outer:                                        ; preds = %for.latch, %entry
  %i = phi i32 [ %add7, %for.latch ], [ 0, %entry ]
  br label %for.inner

for.inner:                                        ; preds = %for.inner, %for.outer
  %j = phi i32 [ %add6, %for.inner ], [ 0, %for.outer ]
  %sum = phi i32 [ %add, %for.inner ], [ 0, %for.outer ]
  %arrayidx5 = getelementptr inbounds i32, ptr %B, i32 %j
  %0 = load i32, ptr %arrayidx5, align 4
  %mul = mul nsw i32 %0, %i
  %add = add nsw i32 %mul, %sum
  %add6 = add nuw nsw i32 %j, 1
  %arrayidx = getelementptr inbounds i32, ptr %A, i32 %i
  store i32 1, ptr %arrayidx, align 4
  %add72 = add nuw nsw i32 %i, 1
  %arrayidx8 = getelementptr inbounds i32, ptr %A, i32 %add72
  store i32 %add, ptr %arrayidx8, align 4
  %exitcond = icmp eq i32 %add6, 100
  br i1 %exitcond, label %for.latch, label %for.inner

for.latch:                                        ; preds = %for.inner
  %add7 = add nuw nsw i32 %i, 1
  %exitcond29 = icmp eq i32 %add7, 100
  br i1 %exitcond29, label %cleanup, label %for.outer

cleanup:                                          ; preds = %for.latch
  ret void
}
