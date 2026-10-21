; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next.phi, %inner.latch ]
  %j.next = add i64 %j, 1
  %gep = getelementptr [256 x i8], ptr %A, i64 %j, i64 %i
  store i8 0, ptr %gep, align 1
  br label %inner.latch

inner.latch:                                      ; preds = %inner.header
  %j.next.phi = phi i64 [ %j.next, %inner.header ]
  %ec.j = icmp eq i64 %j.next, 256
  br i1 %ec.j, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 256
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
