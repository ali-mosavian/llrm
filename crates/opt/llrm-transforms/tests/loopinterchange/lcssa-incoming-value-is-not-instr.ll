; ModuleID = '<stdin>'
source_filename = "<stdin>"

define i32 @f() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 2
  br i1 %ec.j, label %inner.exit, label %inner

inner.exit:                                       ; preds = %inner
  %cst = phi i32 [ 42, %inner ]
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 2
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %cst.lcssa = phi i32 [ %cst, %outer.latch ]
  ret i32 %cst.lcssa
}

define i32 @g() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.latch ]
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %x = phi i32 [ 42, %inner.header ]
  %j.next = add i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 2
  br i1 %ec.j, label %inner.exit, label %inner.header

inner.exit:                                       ; preds = %inner.latch
  %x.lcssa = phi i32 [ %x, %inner.latch ]
  br label %outer.latch

outer.latch:                                      ; preds = %inner.exit
  %x.lcssa2 = phi i32 [ %x.lcssa, %inner.exit ]
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 2
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  %x.final = phi i32 [ %x.lcssa2, %outer.latch ]
  ret i32 %x.final
}
