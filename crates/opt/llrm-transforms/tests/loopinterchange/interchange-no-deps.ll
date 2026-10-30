; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i32 @no_deps_interchange(ptr nocapture %Arr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv19 = phi i64 [ 0, %entry ], [ %indvars.iv.next20, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next, %for2 ]
  %arrayidx6 = getelementptr inbounds [1024 x i32], ptr %Arr, i64 %indvars.iv, i64 %indvars.iv19
  store i32 0, ptr %arrayidx6, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 1024
  br i1 %exitcond, label %for2, label %for1.inc

for1.inc:                                         ; preds = %for2
  %indvars.iv.next20 = add nuw nsw i64 %indvars.iv19, 1
  %exitcond21 = icmp ne i64 %indvars.iv.next20, 1024
  br i1 %exitcond21, label %for1.header, label %exit

exit:                                             ; preds = %for1.inc
  ret i32 0
}

define i32 @no_mem_instrs(ptr %ptr) {
entry:
  br label %for1.header

for1.header:                                      ; preds = %for1.inc, %entry
  %indvars.iv19 = phi i64 [ 0, %entry ], [ %indvars.iv.next20, %for1.inc ]
  br label %for2

for2:                                             ; preds = %for2, %for1.header
  %indvars.iv = phi i64 [ 0, %for1.header ], [ %indvars.iv.next, %for2 ]
  store i64 %indvars.iv, ptr %ptr, align 4
  %indvars.iv.next = add nuw nsw i64 %indvars.iv, 1
  %exitcond = icmp ne i64 %indvars.iv.next, 1024
  br i1 %exitcond, label %for2, label %for1.inc

for1.inc:                                         ; preds = %for2
  %indvars.iv.next20 = add nuw nsw i64 %indvars.iv19, 1
  %exitcond21 = icmp ne i64 %indvars.iv.next20, 1024
  br i1 %exitcond21, label %for1.header, label %exit

exit:                                             ; preds = %for1.inc
  ret i32 0
}
