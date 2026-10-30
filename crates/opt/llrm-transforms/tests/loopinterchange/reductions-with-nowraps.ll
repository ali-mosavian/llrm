; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @reduction_add(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %sum.i = phi i32 [ 0, %entry ], [ %sum.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %sum.j = phi i32 [ %sum.i, %for.i.header ], [ %sum.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %sum.j.next = add nuw nsw i32 %sum.j, %a
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %sum.i.lcssa = phi i32 [ %sum.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}

define void @reduction_mul(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i32 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %prod.i = phi i32 [ 1, %entry ], [ %prod.i.lcssa, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i32 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %prod.j = phi i32 [ %prod.i, %for.i.header ], [ %prod.j.next, %for.j ]
  %idx = getelementptr inbounds [2 x [2 x i32]], ptr %A, i32 0, i32 %j, i32 %i
  %a = load i32, ptr %idx, align 4
  %prod.j.next = mul nuw nsw i32 %prod.j, %a
  %j.inc = add i32 %j, 1
  %cmp.j = icmp slt i32 %j.inc, 2
  br i1 %cmp.j, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %prod.i.lcssa = phi i32 [ %prod.j.next, %for.j ]
  %i.inc = add i32 %i, 1
  %cmp.i = icmp slt i32 %i.inc, 2
  br i1 %cmp.i, label %for.i.header, label %exit

exit:                                             ; preds = %for.i.latch
  ret void
}
