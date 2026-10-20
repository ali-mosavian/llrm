; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @pr43326-triply-nested(ptr noalias %e, ptr noalias %f) {
entry:
  br label %for.outermost.header

for.outermost.header:                             ; preds = %for.outermost.latch, %entry
  %indvars.outermost = phi i64 [ 0, %entry ], [ %indvars.outermost.next, %for.outermost.latch ]
  br label %for.middle.header

for.cond.cleanup:                                 ; preds = %for.outermost.latch
  ret void

for.middle.header:                                ; preds = %for.middle.latch, %for.outermost.header
  %indvars.middle = phi i64 [ 0, %for.outermost.header ], [ %indvars.middle.next, %for.middle.latch ]
  br label %for.innermost

for.outermost.latch:                              ; preds = %for.middle.latch
  %indvars.outermost.next = add nuw nsw i64 %indvars.outermost, 1
  %exitcond.outermost = icmp ne i64 %indvars.outermost.next, 10
  br i1 %exitcond.outermost, label %for.outermost.header, label %for.cond.cleanup

for.middle.latch:                                 ; preds = %for.innermost
  %indvars.middle.next = add nuw nsw i64 %indvars.middle, 1
  %exitcond.middle = icmp ne i64 %indvars.middle.next, 10
  br i1 %exitcond.middle, label %for.middle.header, label %for.outermost.latch

for.innermost:                                    ; preds = %for.innermost, %for.middle.header
  %indvars.innermost = phi i64 [ 0, %for.middle.header ], [ %indvars.innermost.next, %for.innermost ]
  %arrayidx12 = getelementptr inbounds [10 x [10 x i32]], ptr %e, i64 %indvars.innermost, i64 %indvars.middle, i64 %indvars.outermost
  %0 = load i32, ptr %arrayidx12, align 4
  %arrayidx18 = getelementptr inbounds [10 x [10 x i32]], ptr %f, i64 %indvars.innermost, i64 %indvars.middle, i64 %indvars.outermost
  store i32 %0, ptr %arrayidx18, align 4
  %indvars.innermost.next = add nuw nsw i64 %indvars.innermost, 1
  %exitcond.innermost = icmp ne i64 %indvars.innermost.next, 10
  br i1 %exitcond.innermost, label %for.innermost, label %for.middle.latch
}
