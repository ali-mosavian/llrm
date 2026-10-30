; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @inner_header_br_dup(i1 %cond, ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner.latch ]
  br i1 %cond, label %inner.latch, label %inner.latch

inner.latch:                                      ; preds = %inner.header, %inner.header
  %gep = getelementptr [8 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 8
  br i1 %ec.j, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 8
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @inner_header_switch_dup0(i8 %sel, ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner.latch ]
  switch i8 %sel, label %inner.latch [
    i8 0, label %inner.latch
    i8 1, label %inner.body
  ]

inner.body:                                       ; preds = %inner.header
  br label %inner.latch

inner.latch:                                      ; preds = %inner.body, %inner.header, %inner.header
  %gep = getelementptr [8 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 8
  br i1 %ec.j, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 8
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}

define void @inner_header_switch_dup1(i8 %sel, ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner.latch ]
  switch i8 %sel, label %inner.latch [
    i8 0, label %inner.latch
    i8 1, label %inner.latch
  ]

inner.body:                                       ; No predecessors!
  unreachable

inner.latch:                                      ; preds = %inner.header, %inner.header, %inner.header
  %gep = getelementptr [8 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 8
  br i1 %ec.j, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 8
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
