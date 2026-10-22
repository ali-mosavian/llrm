; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(i1 %cond, ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner.latch ]
  br i1 %cond, label %inner.body.0, label %inner.body.1

inner.body.0:                                     ; preds = %inner.header
  br label %inner.latch

inner.body.1:                                     ; preds = %inner.header
  br label %inner.latch

inner.latch:                                      ; preds = %inner.body.1, %inner.body.0
  %gep = getelementptr [42 x i8], ptr %A, i64 %j, i64 %i
  store i8 1, ptr %gep, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 42
  br i1 %ec.j, label %inner.exit, label %inner.header

0:                                                ; No predecessors!
  unreachable

inner.exit:                                       ; preds = %inner.latch
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 42
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
