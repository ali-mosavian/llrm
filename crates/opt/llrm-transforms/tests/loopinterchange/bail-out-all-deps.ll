; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr %A, i64 %val) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.next, %for.j ]
  %subscript.0 = and i64 %i, %val
  %subscript.1 = and i64 %j, %val
  %idx = getelementptr inbounds [4 x i8], ptr %A, i64 %subscript.0, i64 %subscript.1
  store i8 0, ptr %idx, align 1
  %j.next = add nuw nsw i64 %j, 1
  %exit.j = icmp eq i64 %j.next, 4
  br i1 %exit.j, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %i.next = add nuw nsw i64 %i, 1
  %exit.i = icmp eq i64 %i.next, 4
  br i1 %exit.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
