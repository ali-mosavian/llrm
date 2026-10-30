; ModuleID = '<stdin>'
source_filename = "<stdin>"

define noundef float @reduction_reassoc_ninf(ptr %A) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  %sum.i = phi float [ 0.000000e+00, %entry ], [ %sum.i.next, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %sum.j = phi float [ %sum.i, %for.i.header ], [ %sum.j.next, %for.j ]
  %gep = getelementptr [2 x float], ptr %A, i64 %i, i64 %j
  %val = load float, ptr %gep, align 4
  %sum.j.next = fadd reassoc ninf float %sum.j, %val
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 2
  br i1 %ec.j, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %sum.i.next = phi float [ %sum.j.next, %for.j ]
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 2
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  %sum.i.next.lcssa = phi float [ %sum.i.next, %for.i.latch ]
  ret float %sum.i.next.lcssa
}

define void @non_reduction_ninf(ptr %A, ptr %B, ptr noalias %C) {
entry:
  br label %for.i.header

for.i.header:                                     ; preds = %for.i.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %for.i.latch ]
  br label %for.j

for.j:                                            ; preds = %for.j, %for.i.header
  %j = phi i64 [ 0, %for.i.header ], [ %j.inc, %for.j ]
  %gep.A = getelementptr [2 x float], ptr %A, i64 %i, i64 %j
  %val.A = load float, ptr %gep.A, align 4
  %gep.B = getelementptr [2 x float], ptr %B, i64 %i, i64 %j
  %val.B = load float, ptr %gep.B, align 4
  %sum = fadd ninf float %val.A, %val.B
  %gep.C = getelementptr [2 x float], ptr %C, i64 %i, i64 %j
  store float %sum, ptr %gep.C, align 4
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 2
  br i1 %ec.j, label %for.i.latch, label %for.j

for.i.latch:                                      ; preds = %for.j
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 2
  br i1 %ec.i, label %exit, label %for.i.header

exit:                                             ; preds = %for.i.latch
  ret void
}
