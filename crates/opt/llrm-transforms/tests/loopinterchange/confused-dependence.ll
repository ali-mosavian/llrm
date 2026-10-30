; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @may_alias(ptr %p0, ptr %p1) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %idx.0 = getelementptr inbounds [4 x [4 x float]], ptr %p0, i32 0, i32 %j, i32 %i
  %idx.1 = getelementptr inbounds [4 x [4 x float]], ptr %p1, i32 0, i32 %i, i32 %j
  %0 = load float, ptr %idx.0, align 4
  store float %0, ptr %idx.1, align 4
  %j.inc = add nuw nsw i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 4
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %i.inc = add nuw nsw i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 4
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}
