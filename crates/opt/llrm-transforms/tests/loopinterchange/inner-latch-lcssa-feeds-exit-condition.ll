; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @t(i32 %b) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %mid.header

mid.header:                                       ; preds = %mid.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %mid.latch ]
  %mwide = zext i32 %b to i64
  br label %inner.header

inner.header:                                     ; preds = %inner.header, %mid.header
  %k = phi i64 [ 0, %mid.header ], [ %k.next, %inner.header ]
  %k.next = add nsw i64 %k, 1
  %ec.k = icmp eq i64 %k.next, %mwide
  br i1 %ec.k, label %mid.latch, label %inner.header

mid.latch:                                        ; preds = %inner.header
  %j.next = add nsw i64 %j, 1
  %ec.j = icmp eq i64 %j.next, 100
  br i1 %ec.j, label %outer.latch, label %mid.header

outer.latch:                                      ; preds = %mid.latch
  %i.next = add nsw i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 100
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
