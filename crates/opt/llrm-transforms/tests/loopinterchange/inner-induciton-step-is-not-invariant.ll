; ModuleID = '<stdin>'
source_filename = "<stdin>"

define void @step_of_k_is_i_0(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 1, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %k = phi i64 [ 0, %outer.header ], [ %k.next, %inner ]
  %gep = getelementptr [8 x i8], ptr %A, i64 %j, i64 %i
  %old = load i8, ptr %gep, align 1
  %new = add i8 %old, 1
  store i8 %new, ptr %gep, align 1
  %j.next = add i64 %j, 1
  %k.next = add i64 %k, %i
  %ec.inner.not = icmp slt i64 %k, 16
  br i1 %ec.inner.not, label %inner, label %outer.latch

outer.latch:                                      ; preds = %inner
  %i.next = add i64 %i, 1
  %cmp.i = icmp slt i64 %i.next, 8
  br i1 %cmp.i, label %outer.header, label %exit

exit:                                             ; preds = %outer.latch
  ret void
}

define void @step_ok_k_is_i_1(ptr %A) {
entry:
  br label %outer.header

outer.header:                                     ; preds = %outer.latch, %entry
  %i = phi i64 [ 0, %entry ], [ %i.next, %outer.latch ]
  br label %inner

inner:                                            ; preds = %inner, %outer.header
  %j = phi i64 [ 0, %outer.header ], [ %j.next, %inner ]
  %k = phi i64 [ 0, %outer.header ], [ %k.next, %inner ]
  %gep = getelementptr inbounds [40 x i64], ptr %A, i64 %i, i64 %j
  store i64 %k, ptr %gep, align 4
  %j.next = add i64 %j, 1
  %k.next = add i64 %k, %i
  %ec.inner = icmp eq i64 %j, 30
  br i1 %ec.inner, label %outer.latch, label %inner

outer.latch:                                      ; preds = %inner
  %i.next = add i64 %i, 1
  %ec.i = icmp eq i64 %i.next, 6
  br i1 %ec.i, label %exit, label %outer.header

exit:                                             ; preds = %outer.latch
  ret void
}
