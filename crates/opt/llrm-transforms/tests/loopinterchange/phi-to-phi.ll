; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @f(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.inc, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.inc, %inner ]
  %k = phi i64 [ -1, %outer.header ], [ %j, %inner ]
  %gep = getelementptr [10 x i64], ptr %A, i64 %i, i64 %j
  store i64 %k, ptr %gep, align 4
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
