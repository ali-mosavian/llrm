; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @multi_entry_phi(ptr noalias %A, i1 %c) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br i1 %c, label %inner.ph, label %inner.ph

inner.ph:                                         ; preds = %outer.header, %outer.header
  %p = phi i64 [ 42, %outer.header ], [ 42, %outer.header ]
  br label %inner

inner:                                            ; preds = %inner, %inner.ph
  %j = phi i64 [ 0, %inner.ph ], [ %j.inc, %inner ]
  %idx = mul i64 %j, 10
  %off = add i64 %idx, %i
  %gep = getelementptr inbounds i32, ptr %A, i64 %off
  store i32 0, ptr %gep, align 4
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 10
  br i1 %ec.j, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 10
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
