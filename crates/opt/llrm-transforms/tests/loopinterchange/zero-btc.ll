; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @test() {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i8 [ 0, %entry ], [ %.lcssa, %outer.latch ]
  br label %inner.header

inner.header:                                     ; preds = %inner.latch, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner.latch ]
  %0 = phi i8 [ %i, %outer.header ], [ %1, %inner.latch ]
  br label %inner.body

inner.body:                                       ; preds = %inner.body, %inner.header
  br i1 true, label %inner.latch, label %inner.body

inner.latch:                                      ; preds = %inner.body
  %1 = or i8 %0, 0
  %j.next = add i64 %j, 1
  br i1 true, label %outer.latch, label %inner.header

outer.latch:                                      ; preds = %inner.latch
  %.lcssa = phi i8 [ %1, %inner.latch ]
  br i1 true, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
