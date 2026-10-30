; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(i1 %cond) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br i1 %cond, label %outer.latch, label %inner.preheader

inner.preheader:                                  ; preds = %outer.header
  br label %inner

inner:                                            ; preds = %inner.preheader, %inner
  %j = phi i64 [ %j.inc, %inner ], [ 0, %inner.preheader ]
  %j.inc = add i64 %j, 1
  %ec.j = icmp eq i64 %j.inc, 10
  br i1 %ec.j, label %outer.body, label %inner

outer.body:                                       ; preds = %inner
  br label %outer.latch

outer.latch:                                      ; preds = %outer.body, %outer.header
  %p = phi i64 [ %i, %outer.header ], [ 10, %outer.body ]
  %i.inc = add i64 %i, 1
  %ec.i = icmp eq i64 %i.inc, 10
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
