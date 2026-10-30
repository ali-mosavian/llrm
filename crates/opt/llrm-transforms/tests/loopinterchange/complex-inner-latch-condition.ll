; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @complex_inner_latch_cond(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.next, %for.j ]
  %gep = getelementptr [5 x i8], ptr %A, i64 %j, i64 %i
  %old = load i8, ptr %gep, align 1
  %new = add i8 %old, 1
  store i8 %new, ptr %gep, align 1
  %j.next = add i64 %j, 1
  %c0 = icmp slt i64 %j.next, %i
  %c1 = icmp slt i64 %j.next, 100
  %ec.j.not = and i1 %c0, %c1
  br i1 %ec.j.not, label %for.j, label %for.i.latch

for.i.latch:                                      ; preds = %for.j
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 5
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
